#!/bin/bash
mysql -h 127.0.0.1 -P 3307 -u mimir -pmimir_password mimir -e "
SELECT e.id, e.name, e.entity_type, COUNT(r.id) as relation_count
FROM kg_entities e
LEFT JOIN kg_relations r ON e.id = r.from_entity_id OR e.id = r.to_entity_id
WHERE e.name = 'Luo 2014' AND e.source_id = 10
GROUP BY e.id, e.name, e.entity_type;
"
