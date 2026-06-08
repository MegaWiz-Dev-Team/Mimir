-- asgard_studio agents were ungrouped (groups seeded before the tenant existed) →
-- hidden in the dashboard's grouped /agents view. Add 2 groups + assign.
INSERT IGNORE INTO agent_groups (tenant_id, name, display_name, description, sort_order, icon_emoji, color_hex, is_active) VALUES
('asgard_studio','skald-crew','🎬 Skald Crew','กองถ่าย AI: ตรวจคลินิก/ต่อเนื่อง/กำกับ/ตัดต่อ/เขียนพรอมป์/QC',1,'🎬','#7C3AED',1),
('asgard_studio','ep1-cast','🎭 EP1 Cast','ตัวละคร EP1: ผู้ป่วยอิง/พยาบาลอุ้ม/orchestrator/examiner',2,'🎭','#26C0A5',1);
UPDATE agent_configs SET agent_group_id=(SELECT id FROM agent_groups WHERE tenant_id='asgard_studio' AND name='skald-crew')
  WHERE tenant_id='asgard_studio' AND name LIKE 'studio-%';
UPDATE agent_configs SET agent_group_id=(SELECT id FROM agent_groups WHERE tenant_id='asgard_studio' AND name='ep1-cast')
  WHERE tenant_id='asgard_studio' AND name LIKE 'ep1-%';
