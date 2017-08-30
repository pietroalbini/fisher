# Tutorial: auto-deploy from GitHub

Automatically deploying your project from its git repository every time you
push is a great time saver, and one of the most common things done with
webhooks.

In this tutorial we're going to configure Fisher to deploy your project on your
server automatically, every time you push some changes. You need to have
[Fisher installed](install.html) on the machine, and a repository you own on
GitHub.

## Creating the build script

First of all, you need a script that fetches the git repository and deploys it.
This script will be called by Fisher when a new commit is pushed, and its
content depends on how both your repository and your server are structured.

We'll create an example one, which assumes a static site built by running
`make`:

```
#!/bin/bash

git clone https://github.com/username/repository.git src

cd src
make

cp -r build/ /srv/www/example.com
```

While the exact script content will change based on your setup, you might have
noticed no cleanup is done: the cloned repository isn't deleted by the script.
That's intentional, because Fisher automatically runs each script in a
temporary directory, deleting it after.

## Starting Fisher

Now you can start Fisher and play with it! Make sure the build script created
in the previous paragraph is located in its own directory, and the user has the
permissions to execute it. You can then start Fisher with:

```
$ fisher /path/to/the/script/directory
```

If you see the script name in the loaded hooks list, you're good to go! You can
execute the script by doing an HTTP request:

```
$ curl http://localhost:8000/hook/script-name.sh
```

## Integrating with GitHub

Now it's time to integrate your script with GitHub.

Go to your GitHub repository settings and add a new webhook for the `push`
event, pointing to the public URL of Fisher. For example, if the script is
named `deploy.sh` and the public URL of the Fisher instance is
`https://hooks.example.com`, you need to put this URL:

```
https://hooks.example.com/hook/deploy.sh
```

Now it's time to tell Fisher it's integrating with GitHub. The way to do this
is to add a *configuration comment* to the script, a special comment located at
the top of the script, just below the shebang:

```
## Fisher-GitHub: {}
```

The full source code of the deploy script is now:

```
#!/bin/bash
## Fisher-GitHub: {}

git clone https://github.com/username/repository.git src

cd src
make

cp -r build/ /srv/www/example.com
```

After you restart Fisher, it will start recognizing incoming requests from
GitHub, adding during the execution a few useful environment variables.

## Rejecting invalid requests

Right now everyone can start a new deploy, and that might cause issues if
someone finds the URL and starts calling it. Of course you can create a long
and random script name to avoid that, but it's not the cleanest solution.

A better way to fix this is to let GitHub sign requests with a secret key you
provide (I recommend generating a 32 characters random string), and if you tell
Fisher about it all the invalid requests will be rejected.

After you generated the key, go to the GitHub settings for the webhook and copy
the secret key in its field. Then, in the Fisher script, change the
*configuration comment* to this:

```
## Fisher-GitHub: {"secret": "YOUR-SECRET"}
```

After you restart Fisher, all the invalid requests will be rejected!
