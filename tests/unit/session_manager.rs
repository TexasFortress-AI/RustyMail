// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

// Unit tests for SessionManager
// Note: MockSessionManager tests are disabled as MockSessionManager is internal test-only code.
// These tests would need to be refactored to use the public API or moved to integration tests.

#[cfg(test)]
mod tests {
    #[test]
    fn session_manager_module_compiles() {
        // Placeholder so the unit test target keeps this file without unused-import noise.
        let _ = std::any::type_name::<rustymail::session_manager::SessionManager>();
    }
}
