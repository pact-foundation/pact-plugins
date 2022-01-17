To generate the log, run `git log --pretty='* %h - %s (%an, %ad)' TAGNAME..HEAD .` replacing TAGNAME and HEAD as appropriate.

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
