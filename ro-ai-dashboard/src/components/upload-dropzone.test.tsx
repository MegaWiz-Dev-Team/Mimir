import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { UploadDropzone } from './upload-dropzone';

// Helper to create a mock File with a specific size (without actually allocating memory)
function createMockFile(name: string, sizeInBytes: number, type: string): File {
    // Use a small content but override the size property
    const file = new File(['mock'], name, { type });
    Object.defineProperty(file, 'size', { value: sizeInBytes, writable: false });
    return file;
}

// Helper to simulate a drop event with files
function createDropEvent(files: File[]) {
    return {
        dataTransfer: {
            files,
            items: files.map(file => ({
                kind: 'file',
                type: file.type,
                getAsFile: () => file,
            })),
            types: ['Files'],
        },
    };
}

describe('UploadDropzone', () => {
    // UT-F01a: Drop .pdf → File accepted, in list
    it('UT-F01a: accepts a valid .pdf file via drop', async () => {
        const onFilesAdded = jest.fn();
        render(<UploadDropzone onFilesAdded={onFilesAdded} />);

        const dropzone = screen.getByTestId('upload-dropzone');
        const pdfFile = createMockFile('report.pdf', 1024, 'application/pdf');

        // Simulate drop
        fireEvent.drop(dropzone, createDropEvent([pdfFile]));

        await waitFor(() => {
            expect(onFilesAdded).toHaveBeenCalledWith(
                expect.arrayContaining([
                    expect.objectContaining({ name: 'report.pdf' })
                ])
            );
        });
    });

    // UT-F01b: Drop .exe → Rejected + error message
    it('UT-F01b: rejects an .exe file with error message', async () => {
        const onFilesAdded = jest.fn();
        render(<UploadDropzone onFilesAdded={onFilesAdded} />);

        const dropzone = screen.getByTestId('upload-dropzone');
        const exeFile = createMockFile('virus.exe', 1024, 'application/x-msdownload');

        fireEvent.drop(dropzone, createDropEvent([exeFile]));

        await waitFor(() => {
            expect(screen.getByText(/unsupported file type/i)).toBeInTheDocument();
        });

        // onFilesAdded should NOT have been called with the rejected file
        expect(onFilesAdded).not.toHaveBeenCalled();
    });

    // UT-F01c: Drop file > 50MB → Rejected + "File too large"
    it('UT-F01c: rejects a file larger than 50MB with error message', async () => {
        const onFilesAdded = jest.fn();
        render(<UploadDropzone onFilesAdded={onFilesAdded} />);

        const dropzone = screen.getByTestId('upload-dropzone');
        // 51MB file
        const largeFile = createMockFile('huge.pdf', 51 * 1024 * 1024, 'application/pdf');

        fireEvent.drop(dropzone, createDropEvent([largeFile]));

        await waitFor(() => {
            expect(screen.getByText(/file too large/i)).toBeInTheDocument();
        });

        expect(onFilesAdded).not.toHaveBeenCalled();
    });
});
