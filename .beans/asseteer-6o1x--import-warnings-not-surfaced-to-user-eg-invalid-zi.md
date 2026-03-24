---
# asseteer-6o1x
title: Import warnings not surfaced to user (e.g. invalid ZIP archives)
status: todo
type: bug
created_at: 2026-03-23T15:43:31Z
updated_at: 2026-03-23T15:43:31Z
---

During import, warning messages are logged to the backend (e.g. 'Warning: Failed to read zip archive bundle.zip: invalid Zip archive: Could not find EOCD') but never shown to the user. These should be collected and surfaced — either as toast notifications or in a warnings section on the scan result card — so users know which files failed and why.
