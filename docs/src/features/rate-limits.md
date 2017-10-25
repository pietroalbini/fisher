# Rate limits

The Internet is a nasty place, with a bunch of people trying to break into
others' stuff: some of them constantly try to log into things by brute-forcing
passwords and access keys, and that could be a problem if you protect your
hooks with secrets (for example with the [Standalone
provider](../providers/standalone.md)).

To prevent attackers guessing the right secret key by trying all the possible
ones, Fisher supports rate limiting requests built-in. Rate limits are enabled
by default, but they **only affect invalid requests**: you can rest assured all
the legit webhooks will be processed, while the bad ones are automatically
limited.

## Customizing rate limits

By default, Fisher accepts a maximum of 10 invalid requests each minute. While
the default limit should be enough even when you're testing things, you might
need to tweak that.

You can change the default limit with the `--rate-limit` [command-line
argument](../cli.md), for example:

```plain
$ fisher hooks/ --rate-limit 2/1h
```
