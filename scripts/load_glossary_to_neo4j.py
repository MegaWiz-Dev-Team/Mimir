#!/usr/bin/env python3
"""
Load Thai Medical Glossary into Neo4j
Simple loader for glossary.json → Neo4j nodes + relationships
"""

from neo4j import GraphDatabase
import json
from pathlib import Path


def load_glossary(neo4j_uri: str, user: str, password: str, glossary_file: Path):
    """Load glossary JSON into Neo4j"""

    # Connect
    driver = GraphDatabase.driver(neo4j_uri, auth=(user, password))

    # Load JSON
    with open(glossary_file) as f:
        data = json.load(f)

    # Extract abbreviations (metadata at top level)
    if isinstance(data, dict) and 'abbreviations' in data:
        glossary = data['abbreviations']
    else:
        glossary = data

    print(f"📋 Loading {len(glossary)} terms into Neo4j...")

    with driver.session() as session:
        # Create Abbreviation nodes with batch processing
        created_count = 0
        mapped_count = 0

        for abbrev, data in glossary.items():
            # Create Abbreviation node
            session.run("""
                MERGE (a:Abbreviation {abbrev: $abbrev})
                SET a.fullTerm_EN = $full_en,
                    a.fullTerm_TH = $full_th,
                    a.category = $category,
                    a.confidence = $confidence
            """, {
                'abbrev': abbrev,
                'full_en': data.get('fullTerm_EN', ''),
                'full_th': data.get('fullTerm_TH', ''),
                'category': data.get('category', 'General'),
                'confidence': data.get('confidence', 'UNKNOWN')
            })
            created_count += 1

            # If ICD-10-TM mapping exists
            icd10 = data.get('icd10tm')
            if icd10:
                session.run("""
                    MERGE (i:ICD10TM {code: $code})
                    SET i.description = $desc
                """, {
                    'code': icd10,
                    'desc': data.get('fullTerm_EN', '')
                })

                # Create relationship
                session.run("""
                    MATCH (a:Abbreviation {abbrev: $abbrev})
                    MATCH (i:ICD10TM {code: $code})
                    MERGE (a)-[:MAPS_TO_ICD10TM]->(i)
                """, {'abbrev': abbrev, 'code': icd10})
                mapped_count += 1

        print(f"✅ Created {created_count} abbreviation nodes")
        print(f"✅ Created {mapped_count} ICD-10-TM mappings")

        # Verify
        abbrev_count = session.run(
            "MATCH (a:Abbreviation) RETURN COUNT(a) as count"
        ).single()[0]

        icd10_count = session.run(
            "MATCH (i:ICD10TM) RETURN COUNT(i) as count"
        ).single()[0]

        mapping_count = session.run(
            "MATCH (a:Abbreviation)-[:MAPS_TO_ICD10TM]->(i:ICD10TM) RETURN COUNT(*) as count"
        ).single()[0]

        print(f"\n📊 Verification:")
        print(f"   - Abbreviations: {abbrev_count}")
        print(f"   - ICD-10-TM codes: {icd10_count}")
        print(f"   - Mappings: {mapping_count}")

    driver.close()
    print("✅ Neo4j glossary loading complete!")


if __name__ == "__main__":
    load_glossary(
        neo4j_uri='bolt://192.168.194.165:7687',
        user='neo4j',
        password='ba173b8dab83361f13da5aa560419ab607e779b7f4c534b6',
        glossary_file=Path('/Users/mimir/Developer/Mimir/data/abb/glossary.json')
    )
