## Fisher changelog

This document contains what changed in each release of Fisher.

### Next release

*Not yet released.*

**New features:**

* Add the `busy_threads` field to the `GET /health` result

**Changes and improvements:**

* **BREAKING:** Rename fields in the `GET /health` output for consistency:

 * Rename `queue_size` to `queued_jobs`
 * Rename `active_jobs` to `busy_threads`

* Replace the old processor with a faster one
* Improve testing coverage of the project

**Bug fixes:**

* Avoid killing the running jobs when a signal is received

### Fisher 1.0.0-beta.3

*Released on January 5th, 2016.*

* Add the `$FISHER_REQUEST_IP` environment variable
* Add support for status hooks
* Refactored a bunch of the code
* Improve testing coverage of the project

### Fisher 1.0.0-beta.2

*Released on September 24th, 2016.*

* Add support for working behind proxies
* Add support for receiving hooks from **GitLab**
* Show the current Fisher configuration at startup
* Improve unit testing coverage of the project

### Fisher 1.0.0-beta.1

*Released on September 6th, 2016.*

* Initial public release of Fisher
