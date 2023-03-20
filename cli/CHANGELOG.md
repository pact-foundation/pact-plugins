To generate the log, run `git log --pretty='* %h - %s (%an, %ad)' TAGNAME..HEAD .` replacing TAGNAME and HEAD as appropriate.

# 0.1.0 - Support installing known plugins from an index

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


