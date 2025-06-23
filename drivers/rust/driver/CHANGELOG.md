To generate the log, run `git log --pretty='* %h - %s (%an, %ad)' TAGNAME..HEAD .` replacing TAGNAME and HEAD as appropriate.

# 0.7.5 - Maintenance Release

* 289e919 - chore: Upgrade tonic to 0.13.1 (Ronald Holshausen, Mon Jun 23 16:23:19 2025 +1000)
* b2758a0 - chore: Upgrade minor dependencies (Ronald Holshausen, Mon Jun 23 16:13:45 2025 +1000)
* 99c92dc - bump version to 0.7.5 (Ronald Holshausen, Tue Mar 25 16:22:42 2025 +1100)

# 0.7.4 - Maintenance Release

* c64d7c6 - chore: Upgrade crate to Rust 2024 edition (Ronald Holshausen, Tue Mar 25 16:19:44 2025 +1100)
* a97cd0c - chore: Upgrade pact_models to 1.3.0 (Ronald Holshausen, Tue Mar 25 16:17:50 2025 +1100)
* 20ab33f - bump version to 0.7.4 (Ronald Holshausen, Wed Mar 19 11:14:45 2025 +1100)

# 0.7.3 - Bugfix Release

* e7b7b27 - chore: Update dependencies (Ronald Holshausen, Wed Mar 19 10:56:54 2025 +1100)
* 54b76ba - fix: don't override the interaction key if it is already set when calling the verification methods (Ronald Holshausen, Wed Mar 19 10:50:11 2025 +1100)
* e2ca658 - chore: Update doc comment on transport specific configuration must be passed in the test_context (Ronald Holshausen, Fri Dec 13 14:19:01 2024 +1100)
* 08247fd - chore: Upgrade dependencies (Ronald Holshausen, Fri Dec 13 14:17:44 2024 +1100)
* 5d03945 - bump version to 0.7.3 (Ronald Holshausen, Fri Nov 29 15:21:23 2024 +1100)

# 0.7.2 - Bugfix Release

* 5501f24 - chore: update repository.index (Ronald Holshausen, Fri Nov 29 15:19:54 2024 +1100)
* e06d8b3 - fix: When message contents is empty, body is created as Present instead of Empty (Ronald Holshausen, Fri Nov 29 15:00:52 2024 +1100)
* 294f567 - bump version to 0.7.2 (Ronald Holshausen, Tue Sep 3 15:40:42 2024 +1000)

# 0.7.1 - Bugfix Release

* d7871df - chore: update repository.index (Ronald Holshausen, Tue Sep 3 15:39:21 2024 +1000)
* 9c2a76d - chore: Cleanup compiler warning (Ronald Holshausen, Tue Sep 3 15:20:18 2024 +1000)
* b5308af - chore: Cleanup compiler warning (Ronald Holshausen, Tue Sep 3 15:00:36 2024 +1000)
* 88a7f06 - chore: Update dependencies (Ronald Holshausen, Tue Sep 3 10:22:05 2024 +1000)
* c0a0432 - chore: Upgrade pact_models to 1.2.4 (Ronald Holshausen, Tue Sep 3 10:18:58 2024 +1000)
* 70b85f1 - chore: Update dependencies (Ronald Holshausen, Tue Sep 3 10:13:17 2024 +1000)
* 1473cc6 - fix(plugin-driver): use std:process over tokio for plugin loading (Yousaf Nabi, Wed Aug 21 23:55:26 2024 +0100)
* 7b641ae - chore: cleanup trace level span (Ronald Holshausen, Tue Aug 13 10:53:07 2024 +1000)
* 5fc5a4d - chore: Upgrade pact_models to 1.2.2 (Ronald Holshausen, Wed Jul 17 11:18:52 2024 +1000)
* 159fc48 - bump version to 0.7.1 (Ronald Holshausen, Wed Jul 17 09:53:06 2024 +1000)

# 0.7.0 - Upgrade Tonic to 0.12.0 and Prost to 0.13.1

* 482278e - chore: update repository.index (Ronald Holshausen, Wed Jul 17 09:50:15 2024 +1000)
* 550c1cc - chore: Upgrade Tonic to 0.12.0 and Prost to 0.13.1 (Ronald Holshausen, Tue Jul 16 16:08:12 2024 +1000)
* 5622d1f - chore: Upgrade zip crate (Ronald Holshausen, Tue Jul 16 15:53:29 2024 +1000)
* b5401d2 - chore: Upgrade all dependencies (except tonic and prost) (Ronald Holshausen, Tue Jul 16 11:57:25 2024 +1000)
* 8ef58fe - chore(plugin-driver): scope Signal import to unix platforms only (Yousaf Nabi, Fri Jul 5 02:00:52 2024 +0100)
* 21c382a - fix(plugin-driver): shutdown plugins correctly on windows (Yousaf Nabi, Fri Jul 5 01:34:09 2024 +0100)
* 7fe7d92 - Revert "chore: Update reqwest to use openssl as rustls brings in aws-lc-rs" (Ronald Holshausen, Tue Jun 25 11:20:26 2024 +1000)
* 7325bb9 - chore: Update reqwest to use openssl as rustls brings in aws-lc-rs (Ronald Holshausen, Tue Jun 25 10:25:52 2024 +1000)
* c45dc9a - bump version to 0.6.2 (Ronald Holshausen, Tue Apr 23 10:38:27 2024 +1000)

# 0.6.1 - Maintenance Release

* a40de4f - chore: Upgrade pact_models to 1.2.0 (Ronald Holshausen, Tue Apr 23 10:33:57 2024 +1000)
* c3e8e5e - bump version to 0.6.1 (Ronald Holshausen, Tue Apr 16 09:05:24 2024 +1000)

# 0.6.0 - Upgrade Tonic to 0.11.0

* 497b734 - chore: update release script (Ronald Holshausen, Mon Apr 15 15:12:10 2024 +1000)
* da4cfb1 - Merge branch 'v0.5.x' (Ronald Holshausen, Mon Apr 15 15:03:30 2024 +1000)
* c7eec99 - bump version to 0.5.3 (Ronald Holshausen, Mon Apr 15 14:59:08 2024 +1000)
* 590c70b - chore: Fix dependencies after upgrade to tonic (Ronald Holshausen, Mon Apr 15 14:47:55 2024 +1000)
* 84c5ade - chore: Upgrade Tonic to 0.11.0 (Ronald Holshausen, Mon Apr 15 14:09:55 2024 +1000)
* 5309340 - chore: Update dependencies (Ronald Holshausen, Mon Apr 15 11:55:36 2024 +1000)
* 33dc2f2 - chore: Bump minor version (Ronald Holshausen, Mon Apr 15 11:46:54 2024 +1000)

# 0.5.2 - Bugfix Release

* 96d39bb - chore: update repository.index (Ronald Holshausen, Mon Apr 15 14:56:42 2024 +1000)
* f31e4de - fix: Correct import for non-Windows targets (Ronald Holshausen, Mon Apr 15 11:37:42 2024 +1000)
* d9e441e - fix: Tests using plugins on Windows were hanging for 5 minutes (Ronald Holshausen, Mon Apr 15 11:28:57 2024 +1000)
* 2ff974d - bump version to 0.5.2 (Ronald Holshausen, Wed Jan 31 19:02:58 2024 +1100)

# 0.5.1 - Bugfix Release

* e7dcd4c - chore: update repository.index (Ronald Holshausen, Wed Jan 31 19:00:38 2024 +1100)
* 1b54a4a - fix: ensure the Pact has the correct interaction keys before sending verification request to a plugin (Ronald Holshausen, Wed Jan 31 16:35:34 2024 +1100)
* 5d526e3 - bump version to 0.5.1 (Ronald Holshausen, Sat Jan 20 14:29:22 2024 +1100)

# 0.5.0 - Maintenance Release

* a61d1b9 - chore: update repository.index (Ronald Holshausen, Sat Jan 20 14:25:27 2024 +1100)
* 9fcb42a - chore: Upgrade tonic and prost to latest (Ronald Holshausen, Sat Jan 20 14:01:26 2024 +1100)
* 1cf8809 - feat: Add support for atLeast and atMost with matching rule definitions (Ronald Holshausen, Sat Jan 20 13:56:43 2024 +1100)
* 5579e58 - chore: Upgrade dependencies (Ronald Holshausen, Sat Jan 20 08:41:28 2024 +1100)
* a1a07b4 - chore: dump minor version (Ronald Holshausen, Sat Jan 20 08:12:00 2024 +1100)
* 0a422f9 - bump version to 0.4.7 (Ronald Holshausen, Thu Dec 14 10:06:34 2023 +1100)

# 0.4.6 - Maintenance Release

* 28621e3 - chore: update repository.index (Ronald Holshausen, Thu Dec 14 10:04:18 2023 +1100)
* 2979225 - chore: Upgrade dependencies (Ronald Holshausen, Thu Dec 14 09:53:50 2023 +1100)
* aa38e61 - chore: Upgrade dependencies (Ronald Holshausen, Thu Dec 14 09:33:38 2023 +1100)
* dd3359c - feat: support tgz for bundled plugins (Yousaf Nabi, Thu Dec 7 16:09:31 2023 +0000)
* 3d89adf - chore: Add features to readme (Ronald Holshausen, Mon Jul 10 17:18:53 2023 +1000)
* f4f3e1b - bump version to 0.4.6 (Ronald Holshausen, Mon Jul 10 17:08:37 2023 +1000)

# 0.4.5 - Bugfix Release

* b3389e4 - update cargo.lock (Elena Gantner, Wed Jun 21 18:49:46 2023 +0200)
* c12a06f - chore: remove rustls-tls feature from reqwest (Elena Gantner, Wed Jun 21 14:16:32 2023 +0200)
* 94a4dee - chore: add features that map to the pact_model features (Ronald Holshausen, Mon Jun 19 13:03:54 2023 +1000)
* 16063a9 - fix: correct typo in error message (Ronald Holshausen, Mon Jun 5 12:03:17 2023 +1000)
* da4d5c5 - Merge pull request #32 from tbeemster/remove-chrono-default-features (Ronald Holshausen, Wed May 24 10:01:23 2023 +1000)
* e96d662 - Default features are removed for chrono dependency (Timothé Beemster, Tue May 23 16:04:03 2023 +0200)
* 958c03f - bump version to 0.4.5 (Ronald Holshausen, Tue May 23 11:51:54 2023 +1000)

# 0.4.4 - Maintenance Release

* c2bb2a6 - chore: Use "Minimum version, with restricted compatibility range" for all Pact crate versions (Ronald Holshausen, Tue May 23 11:49:28 2023 +1000)
* 88a2e1b - bump version to 0.4.4 (Ronald Holshausen, Mon May 15 13:51:36 2023 +1000)

# 0.4.3 - Maintenance Release

* f9b9ca2 - chore: Update dependecies and upgrade pact models to 1.1 (Ronald Holshausen, Mon May 15 13:29:39 2023 +1000)
* a915437 - bump version to 0.4.3 (Ronald Holshausen, Tue Apr 18 10:58:32 2023 +1000)

# 0.4.2 - Support auto-downloading plugins

* 9424024 - chore: cleanup unused import (Ronald Holshausen, Tue Apr 18 10:55:11 2023 +1000)
* a01455d - fix: only display download progress bar when invoked from CLI (Ronald Holshausen, Tue Apr 18 10:46:58 2023 +1000)
* 639caeb - fix: when auto-installing a plugin, need to correctly set the plugin dir (Ronald Holshausen, Mon Apr 17 16:37:16 2023 +1000)
* f6706d6 - refactor: Move all code to download plugins to the driver to support auto-installing known plugins (Ronald Holshausen, Mon Apr 17 15:57:31 2023 +1000)
* 586aba8 - bump version to 0.4.2 (Ronald Holshausen, Wed Apr 5 16:51:15 2023 +1000)

# 0.4.1 - Bugfix Release

* b84a0ba - revert "feat: Use a shared channel and gRPC client to communicate with a plugin" (Ronald Holshausen, Wed Apr 5 16:48:56 2023 +1000)
* 5fb6107 - chore: Update the generated proto code (Ronald Holshausen, Wed Apr 5 13:50:31 2023 +1000)
* 88cbfc0 - bump version to 0.4.1 (Ronald Holshausen, Tue Apr 4 14:13:50 2023 +1000)

# 0.4.0 - Use shared gRPC plugin client

* 6235b6c - feat: Use a shared channel and gRPC client to communicate with a plugin (Ronald Holshausen, Tue Apr 4 13:47:14 2023 +1000)
* 550c331 - refactor: Use a shared channel to the plugin which is clonable (Ronald Holshausen, Mon Apr 3 15:28:56 2023 +1000)
* 6ed7c4d - refactor: Move plugin repository models to the plugin driver (Ronald Holshausen, Mon Apr 3 11:27:14 2023 +1000)
* 548f9ea - chore: Bump minor version, update all dependecies (esp tonic to 0.9.0) (Ronald Holshausen, Mon Apr 3 10:33:23 2023 +1000)
* ea27510 - bump version to 0.3.4 (Ronald Holshausen, Tue Mar 14 16:57:43 2023 +1100)

# 0.3.3 - Maintenance Release

* 922c57c - chore: bump pact models to 1.0.6 (Ronald Holshausen, Tue Mar 14 16:54:32 2023 +1100)
* 9954abb - chore: fix driver build on musl (Ronald Holshausen, Tue Mar 14 14:06:19 2023 +1100)
* 0a73143 - chore: use log crate, as tracing::log::max_level is not available on Alpine (Ronald Holshausen, Tue Mar 14 13:48:30 2023 +1100)
* 8ae7e7e - fix: require the log feature for tracing crate (Ronald Holshausen, Tue Mar 14 12:12:26 2023 +1100)
* 8e335d5 - bump version to 0.3.3 (Ronald Holshausen, Thu Feb 16 12:00:23 2023 +1100)

# 0.3.2 - Bugfix Release

* 1ffd3ad - fix: InteractionVerificationData fields were not public and there was no constructor function (Ronald Holshausen, Thu Feb 16 11:43:26 2023 +1100)
* d30b769 - chore: fix failing build (Ronald Holshausen, Wed Feb 8 15:55:22 2023 +1100)
* d06f7f8 - chore: check for PACT_DO_NOT_TRACK in both upper and lower case (Ronald Holshausen, Wed Feb 8 14:41:06 2023 +1100)
* 1b7e6ee - bump version to 0.3.2 (Ronald Holshausen, Wed Feb 8 13:22:17 2023 +1100)

# 0.3.1 - Bugfix Release

* abdf9a7 - chore: Update dependencies (Ronald Holshausen, Wed Feb 8 13:19:02 2023 +1100)
* 8d98e65 - feat: add field to mock server mismatches to indicate the type of mismatch (Ronald Holshausen, Tue Feb 7 15:49:21 2023 +1100)
* 4a23448 - bump version to 0.3.1 (Ronald Holshausen, Mon Feb 6 14:47:37 2023 +1100)

# 0.3.0 - Support configuring matching rules and generators for message metadata

* 66a0f01 - feat: Support configuring matching rules and generators for message metadata (Ronald Holshausen, Mon Feb 6 14:20:41 2023 +1100)
* 17c66d9 - bump version to 0.2.3 (Ronald Holshausen, Fri Dec 16 16:25:26 2022 +1100)

# 0.2.2 - Bugfix Release

* 2de4518 - feat: Support passing a test context through to the start mock server call (Ronald Holshausen, Fri Dec 16 16:14:48 2022 +1100)
* 6a4739b - chore: Upgrade tokio to latest (Ronald Holshausen, Fri Dec 16 11:01:40 2022 +1100)
* 7fdc756 - bump version to 0.2.2 (Ronald Holshausen, Wed Dec 14 14:46:27 2022 +1100)

# 0.2.1 - Maintenance Release

* 3cc6f04 - feat: add TestMode and ContentFor to GenerateContentRequest; Implement support in JVM driver (Ronald Holshausen, Wed Dec 14 14:26:22 2022 +1100)
* 256080d - bump version to 0.2.1 (Ronald Holshausen, Fri Dec 9 16:53:12 2022 +1100)

# 0.2.0 - support GenerateContents RPC method

* a61ab86 - feat: Update driver to support GenerateContents RPC method (Ronald Holshausen, Fri Dec 9 15:35:19 2022 +1100)
* 79e5b15 - bump version to 0.1.17 (Ronald Holshausen, Mon Nov 28 13:51:36 2022 +1100)

# 0.1.16 - Maintenance Release

* c4c814e - chore: set pact models to use any 1.0+ version (Ronald Holshausen, Mon Nov 28 13:35:05 2022 +1100)
* 55da8a9 - bump version to 0.1.16 (Ronald Holshausen, Mon Nov 7 11:06:12 2022 +1100)

# 0.1.15 - Maintenance Release

* 93d436c - fix: Upgrade pact models to 1.0 to fix dependency cycle issue (Ronald Holshausen, Mon Nov 7 10:48:52 2022 +1100)
* 211cb85 - bump version to 0.1.15 (Ronald Holshausen, Fri Nov 4 16:09:31 2022 +1100)

# 0.1.14 - Maintenance Release

* 80395c8 - chore: Upgrade pact_models to 0.4.6 (Ronald Holshausen, Fri Nov 4 15:56:26 2022 +1100)
* f9fc661 - bump version to 0.1.14 (Ronald Holshausen, Wed Oct 5 17:23:25 2022 +1100)

# 0.1.13 - Bugfix Release

* 8fbfac2 - fix: Updated load plugin manifest to return the max matching version, not the first one (Ronald Holshausen, Wed Oct 5 16:37:52 2022 +1100)
* 2a26a25 - bump version to 0.1.13 (Ronald Holshausen, Mon Sep 12 17:40:54 2022 +1000)

# 0.1.12 - Bugfix Release

* f059f54 - fix(Rust driver): register_plugin_entries was setting the entry keys incorrectly (Ronald Holshausen, Mon Sep 12 17:14:23 2022 +1000)
* a54765f - chore: run build with latest CSV plugin (Ronald Holshausen, Thu Sep 8 15:11:14 2022 +1000)
* c220ddb - chore: download the plugin cli instead of building it every time (Ronald Holshausen, Thu Sep 8 13:50:24 2022 +1000)
* 96dca91 - bump version to 0.1.12 (Ronald Holshausen, Thu Sep 8 11:21:55 2022 +1000)

# 0.1.11 - Maintenance Release

* 97d8bd4 - feat: Implement update_catalogue call and add function to increment plugin access count (Ronald Holshausen, Thu Sep 8 11:12:58 2022 +1000)
* d638a27 - bump version to 0.1.11 (Ronald Holshausen, Thu Aug 18 14:05:18 2022 +1000)

# 0.1.10 - Maintenance Release

* 5e685c8 - chore: inline the generated protobuf code so the build no longer requires protoc installed (Ronald Holshausen, Thu Aug 18 13:56:10 2022 +1000)
* be7cda2 - chore: Upgrade pact_models and tracing crates (Ronald Holshausen, Thu Aug 18 13:44:55 2022 +1000)
* 95bfb08 - bump version to 0.1.10 (Ronald Holshausen, Wed Aug 10 09:58:06 2022 +1000)

# 0.1.9 - Maintenance Release

* 973aed9 - chore: Upgrade pact models to 0.4.2 (Ronald Holshausen, Wed Aug 10 09:47:20 2022 +1000)
* f44348e - chore: Update musl build (Ronald Holshausen, Mon Aug 8 17:57:34 2022 +1000)
* 77ea0f2 - chore: Update deprecated method after updating dependencies (Ronald Holshausen, Mon Aug 8 15:37:02 2022 +1000)
* c2cbf48 - chore: Update dependent crates (prost, tonic, sysinfo, uuid) (Ronald Holshausen, Mon Aug 8 14:59:58 2022 +1000)
* ab7899a - feat: add support for plugin command line args (Ronald Holshausen, Fri Jul 15 13:28:29 2022 -0400)
* 2515001 - fix: was missing an import (Ronald Holshausen, Fri Jun 3 16:38:39 2022 +1000)
* cdcda73 - feat: add support for installing plugins from Zip files (Ronald Holshausen, Fri Jun 3 16:20:31 2022 +1000)
* 24b25a8 - bump version to 0.1.9 (Ronald Holshausen, Thu May 26 14:14:38 2022 +1000)

# 0.1.8 - Bugfix Release

* 0ecb1cc - fix: log level was not being set correctly (Ronald Holshausen, Thu May 26 12:50:32 2022 +1000)
* 7515dfb - bump version to 0.1.8 (Ronald Holshausen, Fri May 20 15:23:04 2022 +1000)

# 0.1.7 - return mock server results from a running server

* 647cf93 - feat: add method to return mock server results from a running server (Ronald Holshausen, Fri May 20 14:22:50 2022 +1000)
* 105abe1 - feat: added example gRPC consumer example (Ronald Holshausen, Tue May 10 16:57:03 2022 +1000)
* 7a3881f - bump version to 0.1.7 (Ronald Holshausen, Mon May 9 17:18:07 2022 +1000)

# 0.1.6 - replace logging with tracing crate

* 1dbb311 - chore: lock the version of the tracing core crate (Ronald Holshausen, Mon May 9 16:39:13 2022 +1000)
* e194ab9 - chore: replace logging with tracing crate (Ronald Holshausen, Mon May 9 16:27:31 2022 +1000)
* f2e4c3f - chore: Update dependencies (Ronald Holshausen, Mon May 9 14:59:12 2022 +1000)
* 75bf662 - bump version to 0.1.6 (Ronald Holshausen, Tue Apr 26 13:51:05 2022 +1000)

# 0.1.5 - Maintenance Release

* aa04485 - chore: move pact tests into a seperate crate (Ronald Holshausen, Tue Apr 26 13:25:20 2022 +1000)
* 3bb8a49 - feat: support plugins returning user output for verification (Ronald Holshausen, Tue Apr 26 13:19:41 2022 +1000)
* addce70 - bump version to 0.1.5 (Ronald Holshausen, Fri Apr 22 14:40:58 2022 +1000)

# 0.1.4 - Plugin Verification

* f1c147e - feat: add support for plugins verifying interactions (Ronald Holshausen, Fri Apr 22 14:16:30 2022 +1000)
* 2fc040e - chore: Upgrade pact_consumer to 0.9.1 and reqwest to 0.11.10 (Ronald Holshausen, Wed Apr 13 16:23:05 2022 +1000)
* ac47083 - bump version to 0.1.4 (Ronald Holshausen, Wed Apr 13 13:59:17 2022 +1000)

# 0.1.3 - Maintenance Release

* a47a327 - fix: Upgrade pact-models to 0.3.3 (fixes issue with handling bad system DER certs) (Ronald Holshausen, Wed Apr 13 13:52:48 2022 +1000)
* d3865e3 - bump version to 0.1.3 (Ronald Holshausen, Wed Apr 13 12:31:28 2022 +1000)

# 0.1.2 - Bugfix Release

* f543f99 - fix: async functions can not use closures (Ronald Holshausen, Wed Apr 13 11:55:25 2022 +1000)
* e4a8b68 - fix: try an IP4 connection if the IP6 one to the plugin fails (Ronald Holshausen, Wed Apr 13 11:33:44 2022 +1000)
* 83c701f - bump version to 0.1.2 (Ronald Holshausen, Mon Apr 11 17:35:27 2022 +1000)

# 0.1.1 - Supports verifying interactions via plugins

* a01d903 - feat: update interface to return mismatch info from verification call (Ronald Holshausen, Fri Apr 8 14:29:08 2022 +1000)
* 09874d1 - fix: return failed verification error instead of throwing an exception (Ronald Holshausen, Thu Apr 7 12:13:55 2022 +1000)
* 949ec66 - feat: add the pact and integration into the verify call (Ronald Holshausen, Thu Mar 31 11:41:50 2022 +1100)
* 8699571 - feat: interface to verify an interaction via a plugin (Ronald Holshausen, Wed Mar 30 16:46:19 2022 +1100)
* 22606f1 - fix: correct the plugin version check when loading plugins (Ronald Holshausen, Fri Mar 25 11:47:34 2022 +1100)
* 5473a72 - fix: do not include the plugin version in the test (Ronald Holshausen, Thu Mar 24 17:30:48 2022 +1100)
* ee303fe - chore: use the published version of pact_consumer crate (Ronald Holshausen, Thu Mar 24 15:20:44 2022 +1100)
* fda31e6 - Revert "chore: tmp disable Pact tests to resolve cyclic dependency issue" (Ronald Holshausen, Thu Mar 24 15:18:43 2022 +1100)
* 0ca4c28 - bump version to 0.1.1 (Ronald Holshausen, Thu Mar 24 13:29:15 2022 +1100)

# 0.1.0 - Support mock servers from plugins

* 13ee5e1 - chore: tmp disable Pact tests to resolve cyclic dependency issue (Ronald Holshausen, Thu Mar 24 12:47:47 2022 +1100)
* 04d982f - refactor: rename mock-server -> transport in Rust code (Ronald Holshausen, Mon Mar 21 15:57:03 2022 +1100)
* 3259859 - chore: update pact models to 0.3.1 (Ronald Holshausen, Fri Mar 18 14:58:25 2022 +1100)
* a36e2d3 - feat: support for plugins supplying mock servers (Ronald Holshausen, Thu Mar 17 16:32:29 2022 +1100)
* 3624f36 - feat: correct MockServerResult message, remove double repeated field (Ronald Holshausen, Fri Mar 11 14:10:47 2022 +1100)
* 1d3700f - chore: switch from log crate to tracing crate (Ronald Holshausen, Thu Mar 10 14:34:41 2022 +1100)
* bb0b2b3 - feat: Update plugin interface to return the results from the mock server (Ronald Holshausen, Thu Mar 10 14:28:52 2022 +1100)
* 6777c17 - Merge branch 'main' into feat/grpc-mock-server (Ronald Holshausen, Mon Mar 7 10:31:12 2022 +1100)
* 65f680d - chore: update dependencies (Ronald Holshausen, Mon Mar 7 10:28:47 2022 +1100)
* b725d2c - bump version to 0.0.19 (Ronald Holshausen, Fri Mar 4 12:17:25 2022 +1100)
* 3ab2701 - wip: started support for plugins providing mock servers (Ronald Holshausen, Fri Mar 4 11:14:46 2022 +1100)

# 0.0.18 - Upgrade pact-models to 0.3.0

* ca7d4d1 - chore: Upgrade pact-models to 0.3.0 (Ronald Holshausen, Fri Mar 4 12:02:06 2022 +1100)
* 686bbca - chore: Upgrade pact consumer crate to 0.8.5 (Ronald Holshausen, Wed Jan 19 11:30:59 2022 +1100)
* a45fac8 - bump version to 0.0.18 (Ronald Holshausen, Mon Jan 17 11:19:34 2022 +1100)

# 0.0.17 - Bugfix Release

* d97daeb - chore: Upgrade pact-models crate to 0.2.7 (Ronald Holshausen, Mon Jan 17 10:58:01 2022 +1100)
* e871081 - fix: log crate version must be fixed across all pact crates (i.e. pact FFI) (Ronald Holshausen, Fri Jan 14 16:12:48 2022 +1100)
* 883a414 - chore: Update pact_consumer to 0.8.4 (Ronald Holshausen, Tue Jan 4 12:55:06 2022 +1100)
* 676a728 - bump version to 0.0.17 (Ronald Holshausen, Tue Jan 4 09:25:39 2022 +1100)

# 0.0.16 - Maintenance Release

* e0b779d - chore: add some trace statements for looking up content handlers (Ronald Holshausen, Tue Jan 4 09:17:06 2022 +1100)
* 3e58db9 - bump version to 0.0.16 (Ronald Holshausen, Fri Dec 31 15:00:13 2021 +1100)

# 0.0.15 - Bugfix Release

* d3af83b - chore: Upgrade to pact_models 0.2.6 (Ronald Holshausen, Fri Dec 31 14:54:40 2021 +1100)
* 7270c75 - fix: allow plugin versions to differ in patch version (Ronald Holshausen, Fri Dec 31 14:42:44 2021 +1100)
* 8289071 - chore: correct the Protobuf pact tests to correctly reflect the proto file (Ronald Holshausen, Thu Dec 30 14:48:21 2021 +1100)
* 27759eb - chore: Update pact_consumer crate to 0.8.3 (Ronald Holshausen, Thu Dec 23 13:51:02 2021 +1100)
* 950b6ad - bump version to 0.0.15 (Ronald Holshausen, Thu Dec 23 12:50:35 2021 +1100)

# 0.0.14 - Maintenance Release

* 61f3d4c - chore: update pact models to 0.2.5 (Ronald Holshausen, Thu Dec 23 12:03:43 2021 +1100)
* cbf7bd9 - chore: update pact models crate to latest (Ronald Holshausen, Tue Dec 21 13:19:42 2021 +1100)
* f3b4c0d - chore: Update tonic, prost and pact_matching crates (Ronald Holshausen, Mon Dec 20 12:27:35 2021 +1100)
* c5248a9 - bump version to 0.0.14 (Ronald Holshausen, Mon Dec 20 12:05:21 2021 +1100)

# 0.0.13 - Bugfix Release

* 5cec4c6 - fix(metrics): swap uid for cid (Ronald Holshausen, Mon Dec 20 11:12:48 2021 +1100)
* b83fa60 - chore: update to latest pact consumer crate (Ronald Holshausen, Wed Dec 15 14:41:44 2021 +1100)
* 653aa89 - bump version to 0.0.13 (Ronald Holshausen, Tue Dec 14 13:35:26 2021 +1100)

# 0.0.12 - Bugfix Release

* 085de61 - fix: correct the plugin load metric call which needs to be a URL encoded FORM POST (Ronald Holshausen, Fri Dec 10 16:56:11 2021 +1100)
* aa19339 - bump version to 0.0.12 (Ronald Holshausen, Mon Nov 29 12:49:39 2021 +1100)

# 0.0.11 - config section in plugin manifest

* 01c3a5c - feat: support config section in plugin manifest (Ronald Holshausen, Mon Nov 29 12:34:00 2021 +1100)
* fc194db - chore: Update to latest models crate (Ronald Holshausen, Mon Nov 29 12:33:29 2021 +1100)
* 601dd9b - chore: bump version (Ronald Holshausen, Tue Nov 16 16:16:03 2021 +1100)

# 0.0.10 - Fix for race condition in Pact FFI calls

* ab64c95 - chore: add additional trace logs for diagnosing race condition (Ronald Holshausen, Tue Nov 16 15:58:19 2021 +1100)
* 6c46e1c - chore: update to the published pact crates (Ronald Holshausen, Tue Nov 16 14:09:07 2021 +1100)
* 2c7849b - bump version to 0.0.10 (Ronald Holshausen, Tue Nov 16 11:46:23 2021 +1100)

# 0.0.9 - Bugfix Release

* 51e7d78 - chore: test using plugin needs to use multi_thread tokio reactor (Ronald Holshausen, Tue Nov 16 11:28:15 2021 +1100)
* 948218c - chore: update to latest pact models (Ronald Holshausen, Tue Nov 16 10:46:17 2021 +1100)
* 9c95244 - feat: add message FFI test using protobuf plugin (Ronald Holshausen, Wed Nov 10 17:09:32 2021 +1100)
* 9e15c48 - feat: update content manager to expose plugin version (Ronald Holshausen, Tue Nov 9 16:08:30 2021 +1100)
* b12590b - chore: use the non-beta pact libs (Ronald Holshausen, Thu Nov 4 16:22:06 2021 +1100)
* 8d2c092 - bump version to 0.0.9 (Ronald Holshausen, Thu Oct 21 18:07:33 2021 +1100)

# 0.0.8 - Bugfix Release

* eaa3f49 - chore: switch to non-beta pact models version (Ronald Holshausen, Thu Oct 21 17:59:27 2021 +1100)
* cd26cd1 - chore: use the channel from the Rust stdlib (Ronald Holshausen, Wed Oct 20 12:09:55 2021 +1100)
* 522f3f0 - chore: fix build on alpine (Ronald Holshausen, Wed Oct 20 11:51:22 2021 +1100)
* 77218b7 - chore: canonicalize() is broken with Windows absolute paths (Ronald Holshausen, Wed Oct 20 10:01:46 2021 +1100)
* d43b68d - chore: debug windows build (Ronald Holshausen, Wed Oct 20 09:23:47 2021 +1100)
* b1106e6 - chore: update to latest pact consumer crate (Ronald Holshausen, Tue Oct 19 17:56:58 2021 +1100)
* 885963c - bump version to 0.0.8 (Ronald Holshausen, Tue Oct 19 17:01:13 2021 +1100)
* 47cc81d - update changelog for release 0.0.7 (Ronald Holshausen, Tue Oct 19 16:57:19 2021 +1100)
* 2219e31 - chore: cargo manifest was pointing to dev consumer crate (Ronald Holshausen, Tue Oct 19 16:55:04 2021 +1100)

# 0.0.7 - Bugfix Release

* 2219e31 - chore: cargo manifest was pointing to dev consumer crate (Ronald Holshausen, Tue Oct 19 16:55:04 2021 +1100)

# 0.0.7 - Bugfix Release

* 1b4ba6e - fix: update pact-models to fix -> EachValue was outputting the wrong JSON (Ronald Holshausen, Tue Oct 19 16:50:52 2021 +1100)
* 8df13ed - chore: update to the latest pact consumer crate (Ronald Holshausen, Tue Oct 19 12:00:03 2021 +1100)
* 0184113 - bump version to 0.0.7 (Ronald Holshausen, Tue Oct 19 10:34:49 2021 +1100)

# 0.0.6 - Bugfix Release

* 856492a - Revert "fix: making entryPoints optional broke loading on Windows" (Ronald Holshausen, Tue Oct 19 09:00:32 2021 +1100)
* b111439 - chore: debugging windows (Ronald Holshausen, Mon Oct 18 17:56:05 2021 +1100)
* b0df1a8 - fix: making entryPoints optional broke loading on Windows (Ronald Holshausen, Mon Oct 18 16:56:15 2021 +1100)
* 1f8acb8 - fix: entry_points should be optional (Ronald Holshausen, Mon Oct 18 15:05:10 2021 +1100)
* cce4258 - fix: handle content types with attributes (Ronald Holshausen, Mon Oct 18 15:04:38 2021 +1100)
* d34e4fe - bump version to 0.0.6 (Ronald Holshausen, Mon Oct 18 13:36:22 2021 +1100)

# 0.0.5 - Support additional plugin entry points

* 403ccbb - chore: update to the latest pact models crate (Ronald Holshausen, Mon Oct 18 13:29:58 2021 +1100)
* d20b9dc - chore: alpine build on CI was failing do to missing protobuf plugin (Ronald Holshausen, Wed Oct 13 13:04:25 2021 +1100)
* ced8d43 - feat: support additional entry points for other operating systems (i.e. requiring a .bat file for Windows) (Ronald Holshausen, Wed Oct 13 10:26:30 2021 +1100)
* 409be18 - feat: Add protobuf consumer pact test (Ronald Holshausen, Tue Oct 12 16:50:52 2021 +1100)
* c3d1585 - bump version to 0.0.5 (Ronald Holshausen, Tue Oct 12 15:42:47 2021 +1100)

# 0.0.4 - synchronous messages with plugins

* a7c6339 - feat: Support synchronous messages with plugins in Rust (Ronald Holshausen, Tue Oct 12 15:35:02 2021 +1100)
* ceee4f4 - chore: update driver readmes (Ronald Holshausen, Tue Oct 5 16:22:28 2021 +1100)
* 233f68d - bump version to 0.0.4 (Ronald Holshausen, Tue Oct 5 15:27:03 2021 +1100)

# 0.0.3 - Changes for Protobuf plugin

* 7c2c122 - chore: use the published version of Pact models (Ronald Holshausen, Tue Oct 5 15:18:01 2021 +1100)
* cf73204 - feat: working Rust consumer test with Protobuf repeated and map fields (Ronald Holshausen, Wed Sep 29 11:21:14 2021 +1000)
* dce8418 - feat: support returning an error when configuring an interaction from the plugin (Ronald Holshausen, Wed Sep 22 17:53:44 2021 +1000)
* e5ecd93 - refactor: rename ContentTypeOverride -> ContentTypeHint (Ronald Holshausen, Tue Sep 14 15:33:26 2021 +1000)
* 261e155 - bump version to 0.0.3 (Ronald Holshausen, Fri Sep 10 14:30:50 2021 +1000)

# 0.0.2 - interaction markup from plugins + concurrent test access

* 691980a - chore: update pact models version (Ronald Holshausen, Fri Sep 10 14:20:17 2021 +1000)
* dd257e0 - feat: Support access to plugins from concurrent running tests (Ronald Holshausen, Fri Sep 10 13:22:55 2021 +1000)
* 9175d18 - refactor: make interaction markup type explicit (Ronald Holshausen, Thu Sep 9 11:20:26 2021 +1000)
* 893f47e - feat: support getting config and interaction markup from plugins (Ronald Holshausen, Wed Sep 8 16:42:01 2021 +1000)
* e3372b9 - bump version to 0.0.2 (Ronald Holshausen, Fri Sep 3 17:41:18 2021 +1000)

# 0.0.1 - Support for protobuf plugin

* 38b2712 - chore: fix the rust driver build (Ronald Holshausen, Fri Sep 3 17:23:43 2021 +1000)
* cda0043 - chore: update docs (Ronald Holshausen, Fri Sep 3 17:22:14 2021 +1000)
* 84d8175 - chore: update plugin driver docs (Ronald Holshausen, Fri Sep 3 14:49:07 2021 +1000)
* de55fc5 - refactor: change configure_interation to return a struct instead of a tuple (Ronald Holshausen, Fri Sep 3 13:07:32 2021 +1000)
* 1e26b94 - feat: update the proto file with comments and enums were needed (Ronald Holshausen, Thu Sep 2 14:26:45 2021 +1000)
* e7f5477 - feat: support for plugins verifying responses (Ronald Holshausen, Thu Sep 2 11:37:08 2021 +1000)
* e657611 - refactor: rename ConfigureContentsRequest -> ConfigureInteractionRequest (Ronald Holshausen, Mon Aug 30 16:28:36 2021 +1000)
* 8fe5b0c - feat(plugins): allow the plugin to override text/binary of a content type (Ronald Holshausen, Mon Aug 30 11:16:38 2021 +1000)
* d10f41e - chore: correct build script for windows (Ronald Holshausen, Mon Aug 23 15:39:28 2021 +1000)
* 9cb8b03 - chore: correct description (Ronald Holshausen, Mon Aug 23 15:35:32 2021 +1000)
* 250bfd4 - chore: bump version to 0.0.1 (Ronald Holshausen, Mon Aug 23 15:33:32 2021 +1000)
* 952a15c - chore: add readme (Ronald Holshausen, Mon Aug 23 15:22:32 2021 +1000)
* e3d5851 - chore: run musl build on updated docker image (Ronald Holshausen, Mon Aug 23 15:17:17 2021 +1000)
* 003d0c4 - chore: to publish the rust driver, the proto file needs to be included (Ronald Holshausen, Mon Aug 23 14:52:11 2021 +1000)
* 8bfd42d - chore: set proto dir relative to cargo manifest dir (Ronald Holshausen, Mon Aug 23 14:47:45 2021 +1000)

# 0.0.0 - First Release
