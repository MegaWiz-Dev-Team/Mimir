#!/usr/bin/env python3
"""
Neo4j Glossary Lookup Module for Medical Extraction Pipeline
Provides dynamic abbreviation expansion from Neo4j instead of static dictionaries
"""

from neo4j import GraphDatabase
from typing import Dict, Optional, List
import os


class Neo4jGlossaryLookup:
    """Neo4j-backed glossary lookup for medical abbreviations"""

    def __init__(self, uri: str = None, user: str = None, password: str = None):
        """Initialize Neo4j connection"""
        self.uri = uri or os.getenv('NEO4J_URI', 'bolt://192.168.194.165:7687')
        self.user = user or os.getenv('NEO4J_USER', 'neo4j')
        self.password = password or os.getenv('NEO4J_PASSWORD', 'ba173b8dab83361f13da5aa560419ab607e779b7f4c534b6')
        self.driver = None
        self._connect()

    def _connect(self):
        """Establish Neo4j connection"""
        try:
            self.driver = GraphDatabase.driver(self.uri, auth=(self.user, self.password))
            # Test connection
            with self.driver.session() as session:
                session.run("RETURN 1")
            print(f"✅ Connected to Neo4j glossary at {self.uri}")
        except Exception as e:
            print(f"⚠️  Neo4j connection failed: {e}")
            print("   Will fall back to static glossary")
            self.driver = None

    def lookup(self, abbreviation: str) -> Optional[Dict]:
        """
        Look up abbreviation in Neo4j glossary

        Args:
            abbreviation: Medical abbreviation to expand

        Returns:
            Dict with fullTerm_EN, fullTerm_TH, icd10tm, icd9 (if found)
        """
        if not self.driver:
            return None

        try:
            with self.driver.session() as session:
                result = session.run("""
                    MATCH (a:Abbreviation {abbrev: $abbrev})
                    OPTIONAL MATCH (a)-[:MAPS_TO_ICD10TM]->(i:ICD10TM)
                    RETURN
                        a.fullTerm_EN as fullTerm_EN,
                        a.fullTerm_TH as fullTerm_TH,
                        a.category as category,
                        i.code as icd10tm
                """, abbrev=abbreviation)

                record = result.single()
                if record and record['fullTerm_EN']:
                    return {
                        'abbreviation': abbreviation,
                        'fullTerm_EN': record['fullTerm_EN'],
                        'fullTerm_TH': record['fullTerm_TH'],
                        'category': record['category'],
                        'icd10tm': record['icd10tm'],
                        'source': 'neo4j'
                    }
        except Exception as e:
            print(f"⚠️  Lookup failed for {abbreviation}: {e}")

        return None

    def lookup_batch(self, abbreviations: List[str]) -> Dict[str, Dict]:
        """
        Look up multiple abbreviations efficiently

        Args:
            abbreviations: List of abbreviations to expand

        Returns:
            Dict mapping abbreviations to their expansions
        """
        results = {}
        for abbrev in abbreviations:
            result = self.lookup(abbrev)
            if result:
                results[abbrev] = result

        return results

    def get_icd_mapping(self, abbreviation: str) -> Dict[str, str]:
        """
        Get ICD code mapping for abbreviation

        Args:
            abbreviation: Medical abbreviation

        Returns:
            Dict with 'icd10tm' and optionally 'icd9'
        """
        lookup = self.lookup(abbreviation)
        if lookup:
            return {
                'icd10tm': lookup.get('icd10tm'),
                'icd9': lookup.get('icd9'),
            }
        return {'icd10tm': None, 'icd9': None}

    def search_by_category(self, category: str) -> List[Dict]:
        """
        Find all abbreviations in a category

        Args:
            category: Category name (e.g., 'DIAGNOSIS', 'MEDICATION')

        Returns:
            List of matching abbreviations
        """
        if not self.driver:
            return []

        try:
            with self.driver.session() as session:
                result = session.run("""
                    MATCH (a:Abbreviation {category: $category})
                    OPTIONAL MATCH (a)-[:MAPS_TO_ICD10TM]->(i:ICD10TM)
                    RETURN
                        a.abbrev as abbrev,
                        a.fullTerm_EN as fullTerm_EN,
                        a.fullTerm_TH as fullTerm_TH,
                        i.code as icd10tm
                """, category=category)

                return [dict(record) for record in result]
        except Exception as e:
            print(f"⚠️  Category search failed: {e}")
            return []

    def close(self):
        """Close Neo4j connection"""
        if self.driver:
            self.driver.close()


# Singleton instance (thread-safe via lazy loading)
_glossary_instance = None


def get_glossary_lookup() -> Neo4jGlossaryLookup:
    """Get or create singleton glossary lookup instance"""
    global _glossary_instance
    if _glossary_instance is None:
        _glossary_instance = Neo4jGlossaryLookup()
    return _glossary_instance


# Example usage for integration
def test_glossary_integration():
    """Test Neo4j glossary integration"""

    print("=" * 70)
    print("🔍 Neo4j Glossary Integration Test")
    print("=" * 70)

    glossary = get_glossary_lookup()

    # Test 1: Single lookup
    print("\n[Test 1] Single abbreviation lookup:")
    result = glossary.lookup('UTI')
    if result:
        print(f"  UTI → {result['fullTerm_EN']} (ICD-10: {result['icd10tm']})")

    # Test 2: Batch lookup
    print("\n[Test 2] Batch lookup (5 abbreviations):")
    test_abbrevs = ['UTI', 'AKI', 'HT', 'DLP', 'Septic shock']
    results = glossary.lookup_batch(test_abbrevs)
    for abbrev, data in results.items():
        icd = data.get('icd10tm', 'N/A')
        print(f"  {abbrev} → {data['fullTerm_EN']} ({icd})")

    # Test 3: ICD mapping
    print("\n[Test 3] ICD code mapping:")
    mapping = glossary.get_icd_mapping('HT')
    print(f"  HT → ICD-10: {mapping['icd10tm']}, ICD-9: {mapping['icd9']}")

    # Test 4: Category search
    print("\n[Test 4] Search by category (DIAGNOSIS):")
    diagnoses = glossary.search_by_category('DIAGNOSIS')
    print(f"  Found {len(diagnoses)} diagnosis abbreviations")
    for item in diagnoses[:3]:
        print(f"    - {item['abbrev']}: {item['fullTerm_EN']}")

    glossary.close()
    print("\n✅ Integration test complete")


if __name__ == "__main__":
    test_glossary_integration()
