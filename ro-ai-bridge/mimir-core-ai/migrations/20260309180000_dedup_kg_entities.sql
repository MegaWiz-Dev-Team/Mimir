-- Deduplicate kg_entities: keep lowest ID for each (name, entity_type, tenant_id)
-- Then add UNIQUE index to prevent future duplicates

-- Step 1: Delete duplicate entities (keep min ID per name+type+tenant)
-- MariaDB-compatible self-join DELETE
DELETE e1 FROM kg_entities e1
INNER JOIN kg_entities e2
  ON e1.name = e2.name
  AND e1.entity_type = e2.entity_type
  AND e1.tenant_id = e2.tenant_id
  AND e1.id > e2.id;

-- Step 2: Relations referencing deleted entity IDs are CASCADE-deleted by FK

-- Step 3: Add UNIQUE index to prevent future duplicates
ALTER TABLE kg_entities ADD UNIQUE INDEX idx_kg_entity_unique (name(255), entity_type, tenant_id);
