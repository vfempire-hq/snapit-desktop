// ai/mod.rs — on-device intelligence, R·01 skeleton.
//
// M3 R·01 landing scope is deliberately narrow — face detection + face
// embeddings + a semantic search index. Model weights are downloaded on
// first run into <app-config>/models/ so we can ship the app installer
// small and let users see progress. Nothing is bundled inside the
// binary that we cannot rebuild ourselves.
//
// R·02 adds Real-ESRGAN + Restormer + LibRaw for the restoration lane
// and OCR — separate modules below.

pub mod search;

/// Ephemeral status object surfaced by `ai_status`.
#[derive(serde::Serialize)]
pub struct AiStatus {
    pub models_ready: bool,
    pub face_index_built: bool,
    pub semantic_index_built: bool,
    pub last_error: Option<String>,
}

impl Default for AiStatus {
    fn default() -> Self {
        Self {
            models_ready: false,
            face_index_built: false,
            semantic_index_built: false,
            last_error: None,
        }
    }
}
