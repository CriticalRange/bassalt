//! Command encoding utilities

use wgpu_core::id;

/// Command encoder wrapper
pub struct CommandEncoder {
    encoder_id: id::CommandEncoderId,
    is_active: bool,
}

impl CommandEncoder {
    pub fn new(encoder_id: id::CommandEncoderId) -> Self {
        Self {
            encoder_id,
            is_active: true,
        }
    }

    pub fn id(&self) -> id::CommandEncoderId {
        self.encoder_id
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn finish(&mut self) {
        self.is_active = false;
    }
}
