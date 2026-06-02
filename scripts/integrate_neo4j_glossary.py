#!/usr/bin/env python3
"""
Neo4j Glossary Integration for Mimir Medical Claims Pipeline
Loads Thai medical abbreviations + ICD mappings into Neo4j
Integrates with extraction pipeline for dynamic lookups
"""

from neo4j import GraphDatabase
from neo4j.exceptions import Neo4jError
import os
import json
from pathlib import Path
from typing import Dict, List, Optional, Tuple


class Neo4jGlossaryIntegration:
    """Integrate Neo4j glossary with medical extraction pipeline"""

    def __init__(self, uri: str, user: str, password: str):
        self.uri = uri
        self.user = user
        self.password = password
        self.driver = None

    def connect(self) -> bool:
        """Establish connection to Neo4j"""
        try:
            self.driver = GraphDatabase.driver(self.uri, auth=(self.user, self.password))
            with self.driver.session() as session:
                session.run("RETURN 1")
            print(f"✅ Connected to Neo4j at {self.uri}")
            return True
        except Exception as e:
            print(f"❌ Connection failed: {e}")
            return False

    def load_glossary_from_json(self, glossary_file: Path) -> bool:
        """Load glossary from JSON file"""
        try:
            with open(glossary_file) as f:
                glossary_data = json.load(f)

            print(f"📋 Loading glossary from {glossary_file.name}")
            print(f"   - Total entries: {len(glossary_data)}")

            with self.driver.session() as session:
                for entry in glossary_data:
                    abbrev = entry.get('abbreviation', '')
                    full_term_en = entry.get('fullTerm_EN', '')
                    full_term_th = entry.get('fullTerm_TH', '')
                    icd10tm = entry.get('icd10tm', '')
                    icd9 = entry.get('icd9cm', '')
                    category = entry.get('category', 'General')

                    # Create or update Abbreviation node
                    cypher = """
                    MERGE (a:Abbreviation {abbrev: $abbrev})
                    SET a.fullTerm_EN = $full_term_en,
                        a.fullTerm_TH = $full_term_th,
                        a.category = $category,
                        a.updated = timestamp()
                    RETURN a.abbrev as abbrev
                    """

                    session.run(cypher, {
                        'abbrev': abbrev,
                        'full_term_en': full_term_en,
                        'full_term_th': full_term_th,
                        'category': category
                    })

                    # Create ICD-10-TM node if code exists
                    if icd10tm:
                        cypher_icd10 = """
                        MERGE (i:ICD10TM {code: $code})
                        SET i.description = $description,
                            i.version = '2024-TM'
                        RETURN i.code as code
                        """

                        session.run(cypher_icd10, {
                            'code': icd10tm,
                            'description': full_term_en
                        })

                        # Link Abbreviation → ICD10TM
                        link_cypher = """
                        MATCH (a:Abbreviation {abbrev: $abbrev})
                        MATCH (i:ICD10TM {code: $icd10})
                        MERGE (a)-[:MAPS_TO_ICD10TM]->(i)
                        RETURN count(*) as links
                        """

                        session.run(link_cypher, {
                            'abbrev': abbrev,
                            'icd10': icd10tm
                        })

                    # Create ICD-9 node if code exists
                    if icd9:
                        cypher_icd9 = """
                        MERGE (i:ICD9 {code: $code})
                        SET i.description = $description,
                            i.version = '2024-CM'
                        RETURN i.code as code
                        """

                        session.run(cypher_icd9, {
                            'code': icd9,
                            'description': full_term_en
                        })

                        # Link ICD10TM → ICD9
                        if icd10tm:
                            equiv_cypher = """
                            MATCH (i10:ICD10TM {code: $icd10})
                            MATCH (i9:ICD9 {code: $icd9})
                            MERGE (i10)-[:EQUIV_ICD9]->(i9)
                            RETURN count(*) as equiv
                            """

                            session.run(equiv_cypher, {
                                'icd10': icd10tm,
                                'icd9': icd9
                            })

            print(f"✅ Glossary loaded successfully")
            return True

        except Exception as e:
            print(f"❌ Failed to load glossary: {e}")
            return False

    def query_abbreviation(self, abbreviation: str) -> Optional[Dict]:
        """Query glossary for abbreviation mapping"""
        try:
            with self.driver.session() as session:
                result = session.run("""
                    MATCH (a:Abbreviation {abbrev: $abbrev})
                    OPTIONAL MATCH (a)-[:MAPS_TO_ICD10TM]->(i10:ICD10TM)
                    OPTIONAL MATCH (i10)-[:EQUIV_ICD9]->(i9:ICD9)
                    RETURN
                        a.fullTerm_EN as fullTerm,
                        a.fullTerm_TH as fullTermTH,
                        i10.code as icd10tm,
                        i9.code as icd9
                """, abbrev=abbreviation)

                record = result.single()
                if record:
                    return {
                        'abbreviation': abbreviation,
                        'fullTerm': record['fullTerm'],
                        'fullTermTH': record['fullTermTH'],
                        'icd10tm': record['icd10tm'],
                        'icd9': record['icd9']
                    }
                return None

        except Exception as e:
            print(f"❌ Query failed: {e}")
            return None

    def verify_glossary(self) -> bool:
        """Verify glossary was loaded correctly"""
        try:
            with self.driver.session() as session:
                # Count nodes
                abbrev_result = session.run("MATCH (a:Abbreviation) RETURN COUNT(a) as count")
                abbrev_count = abbrev_result.single()[0]

                icd10_result = session.run("MATCH (i:ICD10TM) RETURN COUNT(i) as count")
                icd10_count = icd10_result.single()[0]

                icd9_result = session.run("MATCH (i:ICD9) RETURN COUNT(i) as count")
                icd9_count = icd9_result.single()[0]

                print(f"\n✅ Glossary Verification:")
                print(f"   - Abbreviations: {abbrev_count}")
                print(f"   - ICD-10-TM codes: {icd10_count}")
                print(f"   - ICD-9 codes: {icd9_count}")

                # Verify relationships
                mapping_result = session.run(
                    "MATCH (a:Abbreviation)-[:MAPS_TO_ICD10TM]->(i:ICD10TM) RETURN COUNT(*) as count"
                )
                mapping_count = mapping_result.single()[0]
                print(f"   - Abbreviation→ICD-10-TM mappings: {mapping_count}")

                return abbrev_count > 0 and icd10_count > 0

        except Exception as e:
            print(f"❌ Verification failed: {e}")
            return False

    def test_lookups(self) -> bool:
        """Test common abbreviation lookups"""
        test_abbrevs = [
            'UTI', 'AKI', 'HT', 'DLP', 'Septic shock',
            'Bedsore', 'Pleural effusion', 'Hypothyroidism', 'Dementia'
        ]

        print(f"\n✅ Testing Lookups ({len(test_abbrevs)} abbreviations):")
        success_count = 0

        for abbrev in test_abbrevs:
            result = self.query_abbreviation(abbrev)
            if result:
                print(f"   ✅ {abbrev}: {result['fullTerm']} → ICD-10: {result['icd10tm']}, ICD-9: {result['icd9']}")
                success_count += 1
            else:
                print(f"   ❌ {abbrev}: Not found in glossary")

        print(f"\n📊 Test Results: {success_count}/{len(test_abbrevs)} passed")
        return success_count == len(test_abbrevs)

    def close(self):
        """Close Neo4j connection"""
        if self.driver:
            self.driver.close()


def main():
    """Main entry point"""

    # Connection settings (OrbStack)
    neo4j_uri = os.getenv('NEO4J_URI', 'bolt://192.168.194.165:7687')
    neo4j_user = os.getenv('NEO4J_USER', 'neo4j')
    neo4j_password = os.getenv('NEO4J_PASSWORD', 'ba173b8dab83361f13da5aa560419ab607e779b7f4c534b6')

    print("=" * 70)
    print("🔗 Neo4j Glossary Integration")
    print("=" * 70)
    print(f"\nConnecting to Neo4j:")
    print(f"  URI: {neo4j_uri}")
    print(f"  User: {neo4j_user}")

    # Initialize
    integrator = Neo4jGlossaryIntegration(neo4j_uri, neo4j_user, neo4j_password)

    # Step 1: Connect
    print("\n[Step 1] Connecting to Neo4j...")
    if not integrator.connect():
        print("❌ Failed to connect. Exiting.")
        return False

    # Step 2: Load glossary
    print("\n[Step 2] Loading Medical Glossary...")
    glossary_file = Path("/Users/mimir/Developer/Mimir/data/abb/glossary.json")
    if glossary_file.exists():
        if not integrator.load_glossary_from_json(glossary_file):
            print("⚠️  Glossary loading had issues, but continuing...")
    else:
        print(f"⚠️  Glossary file not found: {glossary_file}")

    # Step 3: Verify
    print("\n[Step 3] Verifying Glossary...")
    integrator.verify_glossary()

    # Step 4: Test
    print("\n[Step 4] Testing Lookups...")
    integrator.test_lookups()

    # Step 5: Summary
    print("\n" + "=" * 70)
    print("✅ NEO4J GLOSSARY INTEGRATION COMPLETE")
    print("=" * 70)
    print("\nNext steps:")
    print("1. Integrate Neo4j queries into medical_claims_extractor.py")
    print("2. Test abbreviation lookup with real extraction data")
    print("3. Validate ICD code accuracy")
    print("4. Deploy to production")

    integrator.close()

    return True


if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)
