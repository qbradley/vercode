# VerCode agent plugin

This skills-only plugin provides [custom-vercodable](skills/custom-vercodable/SKILL.md).
The skill creates and updates handwritten Rust codecs from the complete type graph.
It preserves released data passes and requires exact consumption of future encodings.
The bundle contains no hooks, servers, or automatic installation code.

## Install with GitHub Copilot CLI

Use a CLI release that supports the Agent Plugins 1.0 `plugin.json` format.
After the repository contains this marketplace, register it and install the plugin:

```sh
copilot plugin marketplace add microsoft/vercode
copilot plugin install vercode@vercode-tools
```

The marketplace name is `vercode-tools`. The plugin name is `vercode`.
Registration alone does not install the plugin.

For a local checkout, run these commands from the repository root:

```sh
copilot plugin marketplace add .
copilot plugin install vercode@vercode-tools
```

Local installation reads this checkout live. Edits take effect in the next session.

For temporary use without installation, mount the bundle:

```sh
copilot --plugin-dir ./vercode/agent-plugin
```

From an unpacked crate, use `./agent-plugin` instead.

## Use and update

Check plugin and skill discovery:

```sh
copilot plugin list
copilot skill list
```

Ask the agent to use `custom-vercodable` in create or update mode.
For example: "Use custom-vercodable to create a codec for Packet and its nested types."
The skill can also activate for matching codec tasks.

If an older project or personal copy exists, remove that copy before use.
A duplicate skill name can shadow the installed plugin.

To update a plugin installed from GitHub, run:

```sh
copilot plugin update vercode@vercode-tools
```

## Versions and distribution

This bundle accompanies VerCode 0.8.x.
The skill inspects the resolved crate API before it generates code.
This unreleased 0.8.0 checkout includes typed-field support and macro integration.
The skill retains explicit fixed-offset code for dependencies without `WireField`.
An agent plugin update does not update Cargo dependencies or existing codecs.
A crate update does not update the installed plugin.
For reproducible use, mount the bundle from a pinned repository revision or unpacked crate release.

The crate includes this complete directory. Cargo does not install or activate it.
The repository marketplace points to the same bundle, without a second skill copy.
Maintainer evaluations live outside this directory at `skill-evals/custom-vercodable/` in the repository.
Other hosts can load the bundle if they support Agent Plugins 1.0.
Their installation commands can differ.

The [MIT license](LICENSE.txt) applies to this bundle.
