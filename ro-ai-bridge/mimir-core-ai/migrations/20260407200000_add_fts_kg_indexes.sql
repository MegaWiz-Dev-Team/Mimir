-- Add FULLTEXT index on kg_entities.name for fast graph search
-- This replaces the LIKE '%term%' pattern with MATCH/AGAINST for significantly faster lookups

-- MariaDB/MySQL FULLTEXT requires InnoDB or MyISAM engine
-- Our table uses InnoDB which supports FULLTEXT since MySQL 5.6 / MariaDB 10.0.5

ALTER TABLE kg_entities ADD FULLTEXT INDEX idx_kg_entities_name_ft (name);

-- Also add FULLTEXT on kg_relations.relation_type for type-based searches
ALTER TABLE kg_relations ADD FULLTEXT INDEX idx_kg_relations_type_ft (relation_type);
