#!/usr/bin/env python3
"""
Load Medical Abbreviation Glossary into Mimir Neo4j
Connect to Neo4j, load Cypher scripts, test lookup
"""

from neo4j import GraphDatabase
from neo4j.exceptions import Neo4jError
import os
import sys
from pathlib import Path

class MedicalGlossaryLoader:
    """Load medical glossary into Neo4j"""

    def __init__(self, uri: str, user: str, password: str):
        self.driver = GraphDatabase.driver(uri, auth=(user, password))
        self.session = None

    def connect(self) -> bool:
        """Test connection to Neo4j"""
        try:
            with self.driver.session() as session:
                result = session.run("RETURN 1 as result")
                result.consume()
            print("✅ Connected to Neo4j successfully")
            return True
        except Neo4jError as e:
            print(f"❌ Failed to connect to Neo4j: {e}")
            return False

    def load_cypher_script(self, cypher_file: Path) -> bool:
        """Load Cypher script from file"""
        try:
            cypher_content = cypher_file.read_text(encoding='utf-8')

            # Split statements by semicolon
            statements = [s.strip() for s in cypher_content.split(';') if s.strip()]

            with self.driver.session() as session:
                for i, statement in enumerate(statements, 1):
                    # Skip comments and empty lines
                    if statement.startswith('//') or not statement.strip():
                        continue

                    try:
                        print(f"  [{i}/{len(statements)}] Executing statement...", end='', flush=True)
                        session.run(statement)
                        print(" ✅")
                    except Neo4jError as e:
                        # Some statements might fail if nodes already exist - that's OK
                        if "already exists" in str(e).lower():
                            print(" ⚠️  (already exists)")
                        else:
                            print(f" ❌ Error: {e}")

            print(f"✅ Loaded {cypher_file.name}")
            return True

        except Exception as e:
            print(f"❌ Failed to load {cypher_file.name}: {e}")
            return False

    def verify_glossary(self) -> bool:
        """Verify glossary was loaded correctly"""
        try:
            with self.driver.session() as session:
                # Count nodes
                abbrev_count = session.run("MATCH (a:Abbreviation) RETURN COUNT(a) as count").single()[0]
                icd10_count = session.run("MATCH (i:ICD10TM) RETURN COUNT(i) as count").single()[0]
                icd9_count = session.run("MATCH (i:ICD9) RETURN COUNT(i) as count").single()[0]

                print(f"\n✅ Glossary Verification:")
                print(f"   - Abbreviations: {abbrev_count}")
                print(f"   - ICD-10-TM codes: {icd10_count}")
                print(f"   - ICD-9 codes: {icd9_count}")

                # Verify relationships
                mapping_count = session.run(
                    "MATCH (a:Abbreviation)-[:MAPS_TO_ICD10TM]->(i:ICD10TM) RETURN COUNT(*) as count"
                ).single()[0]
                print(f"   - Abbreviation→ICD-10-TM mappings: {mapping_count}")

                return abbrev_count > 0 and icd10_count > 0

        except Exception as e:
            print(f"❌ Verification failed: {e}")
            return False

    def test_lookup(self, abbreviation: str) -> dict:
        """Test abbreviation lookup"""
        try:
            with self.driver.session() as session:
                result = session.run("""
                    MATCH (a:Abbreviation {abbrev: $abbrev})
                    OPTIONAL MATCH (a)-[:MAPS_TO_ICD10TM]->(i10:ICD10TM)
                    OPTIONAL MATCH (i10)-[:EQUIV_ICD9]->(i9:ICD9)
                    RETURN a.fullTerm_EN as fullTerm, i10.code as icd10, i9.code as icd9
                """, abbrev=abbreviation)

                record = result.single()
                if record:
                    return {
                        'abbreviation': abbreviation,
                        'fullTerm': record['fullTerm'],
                        'icd10tm': record['icd10'],
                        'icd9': record['icd9']
                    }
                else:
                    return {'abbreviation': abbreviation, 'status': 'NOT FOUND'}

        except Exception as e:
            return {'abbreviation': abbreviation, 'error': str(e)}

    def test_lookups(self) -> bool:
        """Test common abbreviations"""
        test_abbrevs = [
            'UTI', 'AKI', 'HT', 'DLP', 'Septic shock',
            'Bedsore', 'Pleural effusion', 'Hypothyroidism', 'Dementia'
        ]

        print(f"\n✅ Testing Lookups ({len(test_abbrevs)} abbreviations):")
        success_count = 0

        for abbrev in test_abbrevs:
            result = self.test_lookup(abbrev)
            if 'error' in result:
                print(f"   ❌ {abbrev}: {result['error']}")
            elif 'status' in result:
                print(f"   ❌ {abbrev}: Not found in glossary")
            else:
                print(f"   ✅ {abbrev}: {result['fullTerm']} → ICD-10: {result['icd10tm']}, ICD-9: {result['icd9']}")
                success_count += 1

        print(f"\n📊 Test Results: {success_count}/{len(test_abbrevs)} passed")
        return success_count == len(test_abbrevs)

    def close(self):
        """Close Neo4j connection"""
        if self.driver:
            self.driver.close()


def main():
    """Main entry point"""

    # Get credentials from environment (with OrbStack defaults)
    neo4j_uri = os.getenv('NEO4J_URI', 'bolt://192.168.194.165:7687')
    neo4j_user = os.getenv('NEO4J_USER', 'neo4j')
    neo4j_password = os.getenv('NEO4J_PASSWORD', 'ba173b8dab83361f13da5aa560419ab607e779b7f4c534b6')

    print("="*70)
    print("🔄 Neo4j Glossary Loader")
    print("="*70)
    print(f"\nConnecting to Neo4j:")
    print(f"  URI: {neo4j_uri}")
    print(f"  User: {neo4j_user}")

    # Initialize loader
    loader = MedicalGlossaryLoader(neo4j_uri, neo4j_user, neo4j_password)

    # Step 1: Connect
    print("\n[Step 1] Connecting to Neo4j...")
    if not loader.connect():
        print("❌ Failed to connect. Exiting.")
        sys.exit(1)

    # Step 2: Load Cypher scripts
    print("\n[Step 2] Loading Cypher scripts...")
    cypher_files = [
        Path("/Users/mimir/Developer/Mimir/data/abb/neo4j_abbreviation_mappings.cypher"),
        Path("/Users/mimir/Developer/Mimir/data/abb/auto_glossary.cypher"),
    ]

    for cypher_file in cypher_files:
        if cypher_file.exists():
            print(f"\nLoading {cypher_file.name}...")
            loader.load_cypher_script(cypher_file)
        else:
            print(f"⚠️  File not found: {cypher_file}")

    # Step 3: Verify
    print("\n[Step 3] Verifying glossary...")
    if not loader.verify_glossary():
        print("⚠️  Verification failed - glossary may be incomplete")

    # Step 4: Test lookups
    print("\n[Step 4] Testing lookups...")
    loader.test_lookups()

    # Step 5: Summary
    print("\n" + "="*70)
    print("✅ NEO4J GLOSSARY LOADING COMPLETE")
    print("="*70)
    print("\nNext steps:")
    print("1. Integrate Neo4j lookup into extraction pipeline")
    print("2. Test E2E extraction with dynamic lookup")
    print("3. Benchmark performance")
    print("4. Deploy to production")

    loader.close()


if __name__ == "__main__":
    main()
