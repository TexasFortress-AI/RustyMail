// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! The legacy /api/v1 REST API is off unless explicitly enabled.

use rustymail::api::rest::{rest_api_v1_enabled, REST_API_V1_ENABLED_ENV};

#[test]
fn rest_api_v1_is_off_unless_enabled() {
    std::env::remove_var(REST_API_V1_ENABLED_ENV);
    assert!(!rest_api_v1_enabled(), "unset");
    for off in ["false", "0", "", "yes", "on"] {
        std::env::set_var(REST_API_V1_ENABLED_ENV, off);
        assert!(!rest_api_v1_enabled(), "{:?}", off);
    }
    for on in ["true", "TRUE", "1"] {
        std::env::set_var(REST_API_V1_ENABLED_ENV, on);
        assert!(rest_api_v1_enabled(), "{:?}", on);
    }
    std::env::remove_var(REST_API_V1_ENABLED_ENV);
}
