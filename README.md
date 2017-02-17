url-handler
===========

An application to do generic custom URL-to-command translation.

It receives an URL from command line and convert it to commands based on
a command template.

Imagine you have a custom application to load a custom file format, but
you would want to create links that will launch this this application
and use over your intranet, this will register OS wide URL handlers
to execute your custom commands(s).

Example:

OpenXYZ is an application that open .xyz files and everyone in your team
has it installed and in a known location. But at the same time not everyone
knows command line options or want to use it or something else.
You know a nice trick, that you can pass a scale option and save some
screen space:

```
OpenXYZ myfile.xyz --scale=25%
```

Then, you would like to send this command to someone in your team over
your whatever means your team uses to communicate (email, chat, etc.)
and you would like it opened with a specific scale when clicking over
your link. So you send a link like:

```
xyz://myfile?scale=25
```

Cool, isn't? Not really, but sometimes this is useful for a better work.

This is a simple and hopefully generic way to achieve this without much
fuss.

So, how should `url-handler` be configured for this example?

First, `url-handler` can manage multiple handlers at once, it is based
on a configurable set of rules in the `url-handler.toml` file. Then it
will scan it for rules based on the `scheme` and convert the path and
queries to do argument replacement over your command template.

For our example, we would have this setting (ie. on Windows):

```toml
[[handler]]
scheme = "xyz"
command = "%XYZ_HOME%\\openxyz.exe"
args = "%1.xyz --scale={scale}"
```

What will happen when `url-handler` receives the string `xyz://myfile?scale=25`?

First it will locate a handler fit by matching the scheme `xyz`, then it
will expand the named parameters and numeric arguments. The query is
`scale=25` so it will expand `{scale}` to `25`. Then, as `/myfile` is
the first path, it will replace `%1` with `myfile`. And finally expand
any environment variable and then run the command.

----

Note that custom url handling may have security issues if done
inappropriately. Be sure to understand how your commands will react to
possible malicious inputs from the cloud.

----

Under development.
