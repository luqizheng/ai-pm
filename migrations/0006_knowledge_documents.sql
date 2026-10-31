-- 0006_knowledge_documents: document identity and Git-backed revisions.
-- Per docs/ai-pm-markdown-knowledge-space.md §四/§七.2: the Markdown file (in the
-- knowledge Git repository) is the authoritative body; these tables only carry
-- identity, canonical path, revision number, commit sha and content hash.
-- Deliberately no column stores the body itself.

CREATE TABLE ai.project_document (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES core.project(id),
    doc_key TEXT NOT NULL CHECK (doc_key ~ '^[A-Za-z0-9._-]+$'),
    doc_type TEXT NOT NULL CHECK (
        doc_type IN ('requirement', 'decision', 'architecture', 'knowledge')
    ),
    canonical_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, doc_key),
    UNIQUE (project_id, canonical_path),
    UNIQUE (project_id, id)
);

CREATE TABLE ai.document_revision (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL,
    document_id UUID NOT NULL,
    revision_no INT NOT NULL CHECK (revision_no > 0),
    git_commit_sha TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (
        status IN ('draft', 'published', 'rejected', 'rolled_back')
    ),
    change_summary TEXT,
    created_by UUID NOT NULL REFERENCES core.principal(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_id, revision_no),
    UNIQUE (document_id, id),
    FOREIGN KEY (project_id, document_id)
        REFERENCES ai.project_document(project_id, id)
);

CREATE INDEX idx_document_revision_document
    ON ai.document_revision (document_id, revision_no DESC);