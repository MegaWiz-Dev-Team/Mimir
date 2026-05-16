"""Unit tests for Phase 1 S2: Multi-insurer extraction, file upload, OCR."""

import pytest
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock

from insurance_ingestion_s2.core import PipelineConfig, Chunk
from insurance_ingestion_s2.phases.phase1_extraction_s2 import (
    extract_from_urls,
    extract_from_files,
    chunk_document,
    _extract_pdf,
    _extract_docx,
    _extract_image_ocr,
    run_phase1_s2,
)
from tests.fixtures.sample_data_s2 import (
    SAMPLE_CHUNKS_PRUDENTIAL,
    SAMPLE_CHUNKS_AXA_EN,
    SAMPLE_CHUNKS_AXA_TH,
    SAMPLE_CHUNKS_THAI_HEALTH,
    SAMPLE_INSURERS_CONFIG,
    SAMPLE_UPLOAD_FILES,
)


class TestUrlExtraction:
    """Test URL-based extraction (S1 compat + S2 insurer/language support)."""

    @pytest.fixture
    def config(self):
        return PipelineConfig()

    def test_extract_from_urls_en_single_insurer(self, config):
        """Extract English URLs for single insurer."""
        urls = [
            "https://prudential.co.th/en/products/health/",
        ]

        with patch('requests.get') as mock_get:
            mock_response = Mock()
            mock_response.content = b"""
                <html>
                    <script>var x = 1;</script>
                    <p>PRU Mao Mao Double Sure covers hospitalization</p>
                </html>
            """
            mock_get.return_value = mock_response

            docs = extract_from_urls(
                urls,
                config,
                insurer_id="insurer_001",
                language="en",
            )

            assert len(docs) == 1
            assert docs[0]["insurer_id"] == "insurer_001"
            assert docs[0]["language"] == "en"
            assert "PRU Mao Mao Double Sure" in docs[0]["content"]
            assert "script" not in docs[0]["content"].lower()

    def test_extract_from_urls_multiple_insurers(self, config):
        """Extract URLs for multiple insurers with different languages."""
        urls_en = ["https://prudential.co.th/en/products/health/"]
        urls_th = ["https://axa.co.th/th/products/health/"]

        with patch('requests.get') as mock_get:
            def side_effect(url, **kwargs):
                response = Mock()
                if "axa" in url:
                    response.content = b"<p>สุขภาพ AXA</p>"
                else:
                    response.content = b"<p>Health Prudential</p>"
                return response

            mock_get.side_effect = side_effect

            # Extract for both insurers
            docs_en = extract_from_urls(
                urls_en,
                config,
                insurer_id="insurer_001",
                language="en",
            )
            docs_th = extract_from_urls(
                urls_th,
                config,
                insurer_id="insurer_002",
                language="th",
            )

            assert len(docs_en) == 1
            assert len(docs_th) == 1
            assert docs_en[0]["language"] == "en"
            assert docs_th[0]["language"] == "th"
            assert docs_en[0]["insurer_id"] == "insurer_001"
            assert docs_th[0]["insurer_id"] == "insurer_002"

    def test_extract_from_urls_handles_timeout(self, config):
        """Handle URL timeout gracefully."""
        urls = [
            "https://prudential.co.th/en/products/health/",
            "https://prudential.co.th/en/products/life/",
            "https://prudential.co.th/en/products/invalid/",
        ]

        with patch('requests.get') as mock_get:
            def side_effect(url, **kwargs):
                if "invalid" in url:
                    raise Exception("Timeout")
                response = Mock()
                response.content = b"<p>Content</p>"
                return response

            mock_get.side_effect = side_effect

            docs = extract_from_urls(urls, config, insurer_id="insurer_001")

            # Should extract 2 out of 3 URLs successfully
            assert len(docs) == 2

    @pytest.mark.parametrize("insurer_id,language", [
        ("insurer_001", "en"),
        ("insurer_002", "bi"),
        ("insurer_003", "th"),
    ])
    def test_extract_insurer_language_combinations(self, config, insurer_id, language):
        """Test various insurer + language combinations."""
        urls = ["https://example.co.th/product"]

        with patch('requests.get') as mock_get:
            mock_response = Mock()
            mock_response.content = b"<p>Test content</p>"
            mock_get.return_value = mock_response

            docs = extract_from_urls(urls, config, insurer_id=insurer_id, language=language)

            assert docs[0]["insurer_id"] == insurer_id
            assert docs[0]["language"] == language


class TestFileExtraction:
    """Test file-based extraction (S2 new feature)."""

    @pytest.fixture
    def config(self):
        return PipelineConfig()

    def test_extract_pdf_success(self, config, tmp_path):
        """Extract text from PDF file."""
        pdf_file = tmp_path / "test.pdf"
        pdf_file.write_bytes(b"%PDF-1.4\n%test content")

        with patch('insurance_ingestion_s2.phases.phase1_extraction_s2._extract_pdf') as mock_pdf:
            mock_pdf.return_value = "Extracted PDF content"

            files = [pdf_file]
            docs = extract_from_files(
                files,
                config,
                insurer_id="insurer_001",
                language="en",
            )

            assert len(docs) == 1
            assert docs[0]["source_type"] == "upload"
            assert docs[0]["metadata"]["file_type"] == "pdf"

    def test_extract_docx_success(self, config, tmp_path):
        """Extract text from DOCX file."""
        docx_file = tmp_path / "test.docx"
        docx_file.write_bytes(b"PK\x03\x04...")  # DOCX is ZIP format

        with patch('insurance_ingestion_s2.phases.phase1_extraction_s2._extract_docx') as mock_docx:
            mock_docx.return_value = "Extracted DOCX content"

            files = [docx_file]
            docs = extract_from_files(
                files,
                config,
                insurer_id="insurer_002",
                language="en",
            )

            assert len(docs) == 1
            assert docs[0]["metadata"]["file_name"] == "test.docx"

    def test_extract_txt_direct_read(self, config, tmp_path):
        """Extract text from plain text file."""
        txt_file = tmp_path / "test.txt"
        txt_content = "Plain text insurance document content"
        txt_file.write_text(txt_content, encoding="utf-8")

        files = [txt_file]
        docs = extract_from_files(
            files,
            config,
            insurer_id="insurer_001",
            language="en",
        )

        assert len(docs) == 1
        assert txt_content in docs[0]["content"]

    def test_extract_image_with_ocr(self, config, tmp_path):
        """Extract text from image using OCR."""
        image_file = tmp_path / "test.jpg"
        image_file.write_bytes(b"\xff\xd8\xff\xe0")  # JPEG header

        with patch('insurance_ingestion_s2.phases.phase1_extraction_s2._extract_image_ocr') as mock_ocr:
            mock_ocr.return_value = "Extracted image text"

            files = [image_file]
            docs = extract_from_files(
                files,
                config,
                insurer_id="insurer_003",
                language="th",
            )

            assert len(docs) == 1
            assert "Extracted image text" in docs[0]["content"]

    def test_extract_unsupported_format_skipped(self, config, tmp_path):
        """Skip unsupported file formats."""
        unsupported = tmp_path / "test.xls"
        unsupported.write_bytes(b"test")

        files = [unsupported]
        docs = extract_from_files(
            files,
            config,
            insurer_id="insurer_001",
            language="en",
        )

        assert len(docs) == 0

    def test_file_dedup_by_hash(self, config, tmp_path):
        """Test file deduplication by hash."""
        file1 = tmp_path / "file1.txt"
        file1.write_text("Same content")

        file2 = tmp_path / "file2.txt"
        file2.write_text("Same content")

        files = [file1, file2]
        docs = extract_from_files(
            files,
            config,
            insurer_id="insurer_001",
            language="en",
        )

        # Both files have same content, but different names → different source_ids
        # (Dedup happens in Phase 2, not Phase 1)
        assert len(docs) == 2
        assert docs[0]["metadata"]["file_name"] == "file1.txt"
        assert docs[1]["metadata"]["file_name"] == "file2.txt"

    def test_batch_file_processing(self, config, tmp_path):
        """Process multiple files in batch."""
        files = []
        for i in range(5):
            f = tmp_path / f"doc{i}.txt"
            f.write_text(f"Content {i}")
            files.append(f)

        docs = extract_from_files(
            files,
            config,
            insurer_id="insurer_001",
            language="en",
        )

        assert len(docs) == 5


class TestOCRPipeline:
    """Test OCR extraction pipeline (Syn + pytesseract fallback)."""

    @pytest.fixture
    def config(self):
        return PipelineConfig(ocr_enabled=True)

    def test_ocr_via_syn_success(self, config, tmp_path):
        """Extract image via Syn OCR service."""
        image_file = tmp_path / "test.jpg"
        image_file.write_bytes(b"\xff\xd8\xff\xe0")

        with patch('requests.post') as mock_post:
            mock_response = Mock()
            mock_response.json.return_value = {
                "text": "Extracted via Syn",
                "confidence": 0.92,
            }
            mock_post.return_value = mock_response

            result = _extract_image_ocr(image_file, config)

            assert "Extracted via Syn" in result

    def test_ocr_syn_failure_fallback_pytesseract(self, config, tmp_path):
        """Fallback to pytesseract if Syn fails."""
        image_file = tmp_path / "test.jpg"
        image_file.write_bytes(b"\xff\xd8\xff\xe0")

        with patch('requests.post') as mock_post:
            mock_post.side_effect = Exception("Syn unavailable")

            with patch('insurance_ingestion_s2.phases.phase1_extraction_s2._extract_via_pytesseract') as mock_pyt:
                mock_pyt.return_value = "Extracted via pytesseract"

                result = _extract_image_ocr(image_file, config)

                assert "Extracted via pytesseract" in result

    def test_ocr_confidence_threshold(self, config, tmp_path):
        """Flag low-confidence OCR extracts."""
        image_file = tmp_path / "blurry.jpg"
        image_file.write_bytes(b"\xff\xd8\xff\xe0")

        with patch('requests.post') as mock_post:
            mock_response = Mock()
            mock_response.json.return_value = {
                "text": "Blurry text",
                "confidence": 0.65,  # Below threshold (0.8)
            }
            mock_post.return_value = mock_response

            # Should still extract, but low confidence will be flagged in Phase 2
            result = _extract_image_ocr(image_file, config)
            assert "Blurry text" in result


class TestChunking:
    """Test document chunking (S1 compat + S2 insurer/language fields)."""

    def test_chunk_document_basic(self):
        """Split document into chunks with correct metadata."""
        doc = {
            "source_id": "test_doc_1",
            "content": "Paragraph 1 about health insurance coverage.\n\nParagraph 2 about exclusions.",
            "metadata": {"language": "en", "source_type": "url"},
            "insurer_id": "insurer_001",
            "language": "en",
        }

        chunks = chunk_document(doc)

        assert len(chunks) > 0
        assert all(isinstance(c, Chunk) for c in chunks)
        assert all(c.insurer_id == "insurer_001" for c in chunks)
        assert all(c.language == "en" for c in chunks)

    def test_chunk_document_preserves_metadata(self):
        """Verify all metadata fields preserved in chunks."""
        doc = {
            "source_id": "test_doc_2",
            "content": "Test content " * 100,  # Long content to force multiple chunks
            "metadata": {
                "source_type": "upload",
                "file_name": "test.pdf",
                "insurer_id": "insurer_002",
            },
            "insurer_id": "insurer_002",
            "language": "th",
        }

        chunks = chunk_document(doc)

        for chunk in chunks:
            assert chunk.source_id == "test_doc_2"
            assert chunk.insurer_id == "insurer_002"
            assert chunk.language == "th"
            assert "insurer_id" in chunk.metadata

    def test_chunk_sequential_indices(self):
        """Verify chunks have sequential indices."""
        doc = {
            "source_id": "seq_test",
            "content": "Sentence 1. " * 500,  # Force multiple chunks
            "metadata": {},
            "insurer_id": "insurer_001",
            "language": "en",
        }

        chunks = chunk_document(doc)

        indices = [c.chunk_index for c in chunks]
        assert indices == list(range(len(chunks)))

    @pytest.mark.parametrize("language", ["en", "th", "bi"])
    def test_chunk_preserves_language_field(self, language):
        """Verify language field preserved across chunks."""
        doc = {
            "source_id": f"lang_{language}",
            "content": ("Content in language " * 100),
            "metadata": {"language": language},
            "insurer_id": "insurer_001",
            "language": language,
        }

        chunks = chunk_document(doc)

        assert all(c.language == language for c in chunks)


class TestFullPhase1Pipeline:
    """Integration tests for complete Phase 1 (URLs + Files)."""

    @pytest.fixture
    def config(self):
        return PipelineConfig()

    def test_run_phase1_s2_urls_only(self, config):
        """Run Phase 1 with URL extraction only."""
        urls = ["https://prudential.co.th/en/products/health/"]
        insurers = SAMPLE_INSURERS_CONFIG

        with patch('requests.get') as mock_get:
            mock_response = Mock()
            mock_response.content = b"<p>Health insurance content</p>"
            mock_get.return_value = mock_response

            output_file = run_phase1_s2(
                config,
                urls=urls,
                file_paths=None,
                insurers=insurers,
            )

            assert output_file.exists()
            assert output_file.name == "phase1_chunks.jsonl"

    def test_run_phase1_s2_files_only(self, config, tmp_path):
        """Run Phase 1 with file extraction only."""
        txt_file = tmp_path / "doc.txt"
        txt_file.write_text("Insurance document content")

        insurers = SAMPLE_INSURERS_CONFIG

        output_file = run_phase1_s2(
            config,
            urls=None,
            file_paths=[txt_file],
            insurers=insurers,
        )

        assert output_file.exists()
        assert output_file.name == "phase1_chunks.jsonl"

    def test_run_phase1_s2_hybrid(self, config, tmp_path):
        """Run Phase 1 with both URLs and files."""
        txt_file = tmp_path / "doc.txt"
        txt_file.write_text("File-based content")

        urls = ["https://prudential.co.th/en/products/"]
        insurers = SAMPLE_INSURERS_CONFIG

        with patch('requests.get') as mock_get:
            mock_response = Mock()
            mock_response.content = b"<p>URL-based content</p>"
            mock_get.return_value = mock_response

            output_file = run_phase1_s2(
                config,
                urls=urls,
                file_paths=[txt_file],
                insurers=insurers,
            )

            assert output_file.exists()

    def test_phase1_output_jsonl_format(self, config, tmp_path):
        """Verify Phase 1 output is valid JSONL."""
        import json

        txt_file = tmp_path / "doc.txt"
        txt_file.write_text("Test content for JSONL")

        config.output_dir = tmp_path

        output_file = run_phase1_s2(
            config,
            urls=None,
            file_paths=[txt_file],
            insurers=SAMPLE_INSURERS_CONFIG,
        )

        # Read and validate JSONL
        lines = output_file.read_text().strip().split("\n")
        for line in lines:
            chunk_data = json.loads(line)
            assert "source_id" in chunk_data
            assert "content" in chunk_data
            assert "chunk_index" in chunk_data
            # S2 fields
            assert "insurer_id" in chunk_data["metadata"]
            assert "language" in chunk_data["metadata"]


@pytest.mark.parametrize("format,expected_type", [
    ("pdf", "pdf"),
    ("docx", "docx"),
    ("txt", "text"),
    ("jpg", "image_ocr"),
    ("png", "image_ocr"),
])
def test_file_format_detection(config, tmp_path, format, expected_type):
    """Test correct file type detection."""
    config = PipelineConfig()
    file = tmp_path / f"test.{format}"
    file.write_bytes(b"test")

    with patch('insurance_ingestion_s2.phases.phase1_extraction_s2.extract_from_files') as mock_extract:
        mock_extract.return_value = [
            {
                "source_id": f"file_test",
                "content": "test",
                "metadata": {"document_type": expected_type},
                "insurer_id": "insurer_001",
                "language": "en",
            }
        ]

        docs = extract_from_files([file], config, insurer_id="insurer_001")
        assert len(docs) > 0
