To generate the log, run `git log --pretty='* %h - %s (%an, %ad)' TAGNAME..HEAD .` replacing TAGNAME and HEAD as appropriate.

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
