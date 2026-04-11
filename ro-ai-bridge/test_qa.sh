#!/bin/bash
mysql -h 127.0.0.1 -P 3307 -u mimir -pREDACTED-PW mimir -e "
SELECT COUNT(*) FROM qa_results WHERE source_id = 10;
"
