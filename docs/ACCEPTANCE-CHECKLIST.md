# Manual acceptance checklist (real Samsung TV) — NOT YET EXECUTED

Per OS (Windows 10/11, macOS ARM64, macOS Intel, Ubuntu/Debian): record TV model/year, Tizen version, TV Deploy build, result.

- [ ] 1. Point TV Deploy to an existing Tizen SDK; status becomes "ready".
- [ ] 2. "Find my TV" lists the TV (Developer Mode on).
- [ ] 3. Connect shows Connected only after sdb confirms; DUID is displayed and matches Tizen Studio / `sdb capability`.
- [ ] 4. Wrong IP → "TV doesn't answer" message. 5. Developer Mode off → "refuses developer connections" + correct Host PC IP shown.
- [ ] 6. Wrong port → port-closed message. 7. Host PC IP mismatch → connection-rejected message (record actual wording from sdb).
- [ ] 8. Valid unsigned .wgt → analysed, "Not signed". Signed .wgt → "Signed (not verified yet)".
- [ ] 9. Zip-slip / non-zip / empty / wrong-extension files are rejected with clear messages.
- [ ] 10. Drag-and-drop and file dialog both work. 11. Light/dark/system theme; ru/en.
- [ ] 12. Cancel during scan/connect leaves no stuck process. 13. Restart: saved TV shown but NOT marked connected.
(Signing, install, re-install, incompatible packages: iteration 3 checklist.)
