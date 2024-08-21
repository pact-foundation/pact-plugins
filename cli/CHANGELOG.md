To generate the log, run `git log --pretty='* %h - %s (%an, %ad)' TAGNAME..HEAD .` replacing TAGNAME and HEAD as appropriate.

# 0.1.3 - Adds PACT_PLUGIN_CLI_SKIP_LOAD support + linux aarch64 target

* 37a0610 - chore: update repository.index (Ronald Holshausen, Wed Aug 21 12:00:29 2024 +1000)
* 3e7e007 - chore(plugin-cli): Upgrade dependencies (Ronald Holshausen, Wed Aug 21 11:58:40 2024 +1000)
* 9dc6d01 - feat(cli): support PACT_PLUGIN_CLI_SKIP_LOAD (Yousaf Nabi, Wed Jul 17 23:37:57 2024 +0100)
* 16c6f30 - fix(CLI): use the published version of the driver crate (Ronald Holshausen, Tue Jul 16 12:01:11 2024 +1000)
* 08c7594 - chore(cli): point to local pact-plugin-driver (Yousaf Nabi, Fri Jul 5 01:07:47 2024 +0100)
* fa6ef38 - deps(cli): update deps (Yousaf Nabi, Thu Jul 4 23:15:40 2024 +0100)
* 7c2e691 - docs(chore): pact-plugin-cli 0.0.4 added linux aarch64 target (Yousaf Nabi, Mon May 20 13:27:52 2024 +0100)
* 2a962be - docs(chore): add binary compat table (csv/plugin-cli) (Yousaf Nabi, Mon May 20 13:25:41 2024 +0100)
* 1f3ac8c - bump version to 0.1.1 (pact-plugin-cli) (Yousaf Nabi, Fri May 10 15:21:06 2024 +0100)

# 0.1.2 - Feature Release

* cd8a64a - chore: update repository.index (Yousaf Nabi, Fri May 10 14:52:15 2024 +0100)
* 18dbf5c - feat: reduce executable size (Yousaf Nabi, Fri Apr 26 15:46:58 2024 +0100)
* 20c8dae - feat: linux musl static bins / windows aarch64 (Yousaf Nabi, Thu Apr 25 19:26:03 2024 +0100)
* 53cc657 - chore: fix build after updating version (Ronald Holshausen, Sat Jan 20 08:15:55 2024 +1100)
* 8718c9e - chore: fix build after updating version (Ronald Holshausen, Sat Jan 20 08:14:36 2024 +1100)
* 710f70a - bump version to 0.1.2 (Ronald Holshausen, Thu Dec 14 10:36:50 2023 +1100)

# 0.1.1 - Maintenance Release

* f5d40d1 - chore: update repository.index (Ronald Holshausen, Thu Dec 14 10:34:21 2023 +1100)
* 45866c1 - chore: Upgrade dependencies (Ronald Holshausen, Thu Dec 14 10:21:16 2023 +1100)
* dd3359c - feat: support tgz for bundled plugins (Yousaf Nabi, Thu Dec 7 16:09:31 2023 +0000)
* 20a925c - fix(cli): --skip-if-installed always skipped (Matt Fellows, Tue Oct 3 12:18:13 2023 +1100)
* a01455d - fix: only display download progress bar when invoked from CLI (Ronald Holshausen, Tue Apr 18 10:46:58 2023 +1000)
* f6706d6 - refactor: Move all code to download plugins to the driver to support auto-installing known plugins (Ronald Holshausen, Mon Apr 17 15:57:31 2023 +1000)
* 6ed7c4d - refactor: Move plugin repository models to the plugin driver (Ronald Holshausen, Mon Apr 3 11:27:14 2023 +1000)
* a282599 - bump version to 0.1.1 (Ronald Holshausen, Mon Mar 20 18:00:59 2023 +1100)
* 1456eef - chore: update release script (Ronald Holshausen, Mon Mar 20 17:59:30 2023 +1100)

# 0.1.0 - Support installing known plugins from an index

* 9c0077b - chore: update build script for repository index (Ronald Holshausen, Mon Mar 20 17:51:06 2023 +1100)
* f5dec51 - chore: index file must be local to the package to be able to publish to crates (Ronald Holshausen, Mon Mar 20 17:45:42 2023 +1100)
* cddcd59 - Revert "update changelog for release 0.1.0" (Ronald Holshausen, Mon Mar 20 17:41:49 2023 +1100)
* fdc2d6b - update changelog for release 0.1.0 (Ronald Holshausen, Mon Mar 20 17:36:49 2023 +1100)
* 1064895 - chore: index file must be local to the package to be able to publish to crates (Ronald Holshausen, Mon Mar 20 17:28:22 2023 +1100)
* 433db2a - Revert "update changelog for release 0.1.0" (Ronald Holshausen, Mon Mar 20 17:26:45 2023 +1100)
* 98fb79b - update changelog for release 0.1.0 (Ronald Holshausen, Mon Mar 20 17:16:11 2023 +1100)
* 83dd0fb - feat: add ability to install plugin from index (Ronald Holshausen, Mon Mar 20 17:05:09 2023 +1100)
* b872b15 - feat: update the install CLI to be able to install known plugins (Ronald Holshausen, Mon Mar 20 16:19:29 2023 +1100)
* bc3668f - chore: add CLI snapshots (Ronald Holshausen, Mon Mar 20 16:09:37 2023 +1100)
* ed79701 - feat: split the list command into installed vs known plugins (Ronald Holshausen, Mon Mar 20 15:58:53 2023 +1100)
* 8aee59e - chore: add optimistic locking to repository file access (Ronald Holshausen, Mon Mar 20 14:19:41 2023 +1100)
* 27b9b82 - feat: add CLI command to add all versions from GitHub releases to the index (Ronald Holshausen, Tue Mar 14 10:48:10 2023 +1100)
* 0a54e51 - feat: add optional to add manifest to index from GitHub release (Ronald Holshausen, Fri Mar 10 16:40:52 2023 +1100)
* 17c0e9d - feat: add commands to list entries and versions in the index (Ronald Holshausen, Fri Mar 10 16:06:17 2023 +1100)
* b9dbdcc - feat: need to store the source of the manifest (Ronald Holshausen, Fri Mar 10 15:43:25 2023 +1100)
* 08b3bac - feat: add SHA calculation to repository commands (Ronald Holshausen, Fri Mar 10 15:14:07 2023 +1100)
* eaba5fc - feat: add command to add a plugin version to the index (Ronald Holshausen, Fri Mar 10 14:22:56 2023 +1100)
* 967e2b8 - feat: add local time output to validate command (Ronald Holshausen, Fri Mar 10 13:29:43 2023 +1100)
* 4d8f165 - feat: add validate entry command (Ronald Holshausen, Fri Mar 10 13:18:56 2023 +1100)
* 51bec14 - feat: add initial repository sub-commands (Ronald Holshausen, Fri Mar 10 12:48:02 2023 +1100)
* f50d1b9 - chore: remove trycmd block as there is no way to hide the exit code assertion (Ronald Holshausen, Fri Mar 10 09:39:07 2023 +1100)
* 026f4fb - feat: add option to skip installing a plugin if it is already installed (Ronald Holshausen, Thu Mar 9 17:27:21 2023 +1100)
* fc3c0ea - chore: version arg is optional (Ronald Holshausen, Thu Mar 9 16:55:27 2023 +1100)
* 60c55ef - chore: upgrade clap to v4 (Ronald Holshausen, Thu Mar 9 16:47:04 2023 +1100)
* ac7c88e - chore: skip CLI snapshot tests on Windows (Ronald Holshausen, Thu Mar 9 15:56:15 2023 +1100)
* a27b918 - chore: add CLI snapshot tests (Ronald Holshausen, Thu Mar 9 15:52:34 2023 +1100)
* 8a80e21 - chore: Add CLI tests (Ronald Holshausen, Thu Mar 9 15:11:57 2023 +1100)
* 543fec4 - bump version to 0.0.5 (Ronald Holshausen, Wed Dec 21 16:06:04 2022 +1100)

# 0.0.4 - ARM 64 target

* 5b5ddea - feat: Add ARM 64 target (Ronald Holshausen, Wed Dec 21 15:46:06 2022 +1100)
* ab5d381 - bump version to 0.0.4 (Ronald Holshausen, Tue Dec 20 15:33:59 2022 +1100)

# 0.0.3 - Bugfix Release

* 7e37d08 - fix(windows): force tokio runtime to shutdown as it causes the installation to hang on Windows (Ronald Holshausen, Tue Dec 20 14:51:10 2022 +1100)
* 7e22803 - bump version to 0.0.3 (Ronald Holshausen, Tue Dec 20 12:38:11 2022 +1100)

# 0.0.2 - Build CLI with musl

* eebb4f4 - chore: Upgrade all dependencies (Ronald Holshausen, Tue Dec 20 12:05:47 2022 +1100)
* f44348e - chore: Update musl build (Ronald Holshausen, Mon Aug 8 17:57:34 2022 +1000)
* 1e06bba - bump version to 0.0.2 (Ronald Holshausen, Fri Jun 3 16:29:58 2022 +1000)

# 0.0.1 - Support plugins in Zip files

* cdcda73 - feat: add support for installing plugins from Zip files (Ronald Holshausen, Fri Jun 3 16:20:31 2022 +1000)
* 0708533 - chore: bump patch version (Ronald Holshausen, Fri Jun 3 14:05:54 2022 +1000)
* 3bea3d8 - feat: add -y flag to install and remove command (Ronald Holshausen, Fri Jun 3 14:04:58 2022 +1000)
* 1311292 - chore: Update readme for plugin cli (Ronald Holshausen, Fri Jun 3 13:59:31 2022 +1000)

# 0.0.0 - Initial Release


