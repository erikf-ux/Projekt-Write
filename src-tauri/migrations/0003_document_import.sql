-- Migration 0003: original file name of imported documents (master prompt,
-- section 9, "Import in die Bibliothek", step 4: "Metadaten schreiben,
-- darunter Originaldateiname und Importordner").
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- Phase 3 imports documents by drag and drop. Dropped files arrive without
-- their folder (the webview only reveals the name), so the import folder
-- follows with the folder import.
ALTER TABLE documents ADD COLUMN original_name TEXT
    CHECK (original_name IS NULL OR length(original_name) BETWEEN 1 AND 255);
