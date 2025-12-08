-- Make node_id nullable in all state tables to match Rust struct definitions
ALTER TABLE streams ALTER COLUMN node_id DROP NOT NULL;
ALTER TABLE recordings ALTER COLUMN node_id DROP NOT NULL;
ALTER TABLE ai_tasks ALTER COLUMN node_id DROP NOT NULL;

-- Ensure frames_processed and detections_made are NOT NULL
ALTER TABLE ai_tasks ALTER COLUMN frames_processed SET NOT NULL;
ALTER TABLE ai_tasks ALTER COLUMN detections_made SET NOT NULL;
