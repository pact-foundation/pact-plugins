# Support Python as a second script language plugin (Draft)

Discussion for this proposal: TBD

Related: [002 Support script language plugins](./002_Support_script_language_plugins.md) (Lua)

## Summary

Add Python as a second embedded scripting language for plugins, alongside the Lua support delivered under
proposal 002. The goal is not Python specifically so much as proving that the script-plugin approach is a
generic framework rather than something that only happens to work for Lua.

## Motivation

Proposal 002 shipped Lua support and a working JWT reference plugin. Python was the other language identified
in that proposal as a good candidate for script plugins, and is a natural second implementation to build out:
it is a much more common language among the wider group of people proposal 002 hoped plugins would be
authored by. Before committing to it, we need to resolve the two open questions proposal 002 raised for
Python specifically: system dependencies, and JVM support.

## Background: the bar Lua's design set

Both drivers embed Lua without requiring a system Lua installation:

* **Rust driver**: `mlua` with its `vendored` feature compiles the real Lua 5.4 C source directly into the
  driver binary (`drivers/rust/driver/Cargo.toml`). No external Lua dependency, no version-matching concerns,
  full Lua 5.4 semantics.
* **JVM driver**: `party.iroiro.luajava:lua54-platform:natives-desktop` bundles precompiled native Lua
  libraries *inside the jar*. It is a JNI binding, but to a bundled binary, not a system-installed one
  (`drivers/jvm/core/build.gradle`).

Neither driver asks a user to install anything beyond the plugin itself. This zero-system-dependency property
is the bar a second language should be measured against, not just "can it be embedded at all."

The reference JWT plugin also establishes two further patterns worth carrying forward:

* **Host functions for what the language can't do itself.** Lua has no crypto standard library, so RSA
  sign/verify and base64 are exposed as host (Rust) functions the plugin script calls
  (`register_host_functions` in `lua_plugin.rs`) rather than being implemented in Lua.
* **Vendoring pure dependencies directly into the plugin directory.** The JWT plugin's `base64.lua`,
  `json.lua`, and `inspect.lua` are hand-vendored source files sitting in `plugins/jwt/`, not fetched via
  LuaRocks. LuaRocks support (see below) exists as a second, opt-in mechanism for external/shared
  dependencies, but the reference plugin itself just vendors.

## Driver options

### Rust driver

| Option | Self-contained? | Notes |
|---|---|---|
| **PyO3** (`auto-initialize`) | No | Dynamically links a system `libpython`; the version must match at build *and* run time. Mature, full CPython/pip ecosystem compatibility. |
| **PyO3 static embed** | In theory | PyO3 has no first-class support for this — [issue #416](https://github.com/PyO3/pyo3/issues/416) has been open for years. Would need PyOxidizer on top: a heavy, fragile addition, not a `vendored`-style cargo feature flip. |
| **RustPython** | Yes | Pure Rust, compiles directly into the driver binary like vendored Lua — no system dependency at all. Explicitly not production-ready, and has no C-extension support (no `cryptography`, no compiled wheels of any kind), and real stdlib/language coverage gaps versus CPython. |

### JVM driver

| Option | Self-contained? | Notes |
|---|---|---|
| **GraalPy** (`org.graalvm.python:python-embedding`) | Yes | Runs on plain OpenJDK/Oracle JDK, not just a full GraalVM install. Truffle is pure JVM bytecode — no native artifacts. Much closer to real CPython semantics than RustPython; C-extension support is improving but still experimental. Heavier dependency footprint than `luajava`. |
| **Jep** | No | JNI to a real system CPython. Requires a matching Python installation (and headers, to build) on every machine that runs the driver — reintroduces exactly the system dependency Lua's approach avoided. |
| **Jython** | Yes (moot) | Pure JVM, but stuck at Python 2.7 and unmaintained. Not viable for modern Python. |

### The asymmetry

There is no pairing that is simultaneously self-contained *and* full-CPython-fidelity on both drivers:

* **RustPython + GraalPy** — both zero-system-dependency, matching the bar Lua set. But RustPython's
  incompleteness means Python behaves differently across the two drivers for anything beyond simple
  language/stdlib usage, and real-world PyPI packages with C extensions are unusable on the Rust side.
* **PyO3 + Jep** — both real CPython, so behaviour is consistent across drivers and the full pip ecosystem
  is available. But both require the host machine to have a matching Python installed, which is precisely the
  "System dependencies" risk proposal 002 flagged for Python, and is a regression from what Lua achieved. A
  script plugin that still needs a pre-installed language runtime loses much of its appeal over a plain `exec`
  plugin.

## Recommended approach

RustPython (Rust driver) + GraalPy (JVM driver), to preserve the self-contained, install-nothing-extra
property that is the actual point of the script-plugin model. This is a deliberate trade of ecosystem/compiled-package
compatibility for architectural consistency with Lua. If a prototype shows RustPython's compatibility gaps are
too large in practice, PyO3 (accepting the system dependency) is the fallback for the Rust side; GraalPy's
position does not change either way, as it is the stronger option on the JVM side regardless.

## Package management: pure-Python packages

Proposal 002 raised Python's package management as a specific risk ("Python requires dependencies to be
installed in a particular manner"). The Lua answer — LuaRocks path support plus vendoring — transfers directly:

### Vendoring (primary mechanism, no config needed)

Reserve a conventional subdirectory, e.g. `<plugin_dir>/site-packages`, that both drivers always add to the
interpreter's module search path if present, the same way `<script_dir>` is unconditionally added for Lua.
Plugin authors populate it themselves before distributing the plugin, using:

```
pip install --target <plugin_dir>/site-packages --no-deps --only-binary=:all: \
  --python-version <X.Y> --implementation py --abi none --platform any <package>
```

This is the standard trick for forcing pip to resolve only universal (`py3-none-any`) wheels — the
closest equivalent to LuaRocks' structural `share/lua` vs `lib/lua` split, which pip has no built-in
equivalent of. It fails cleanly if a package has no pure wheel available.

Deliberately **not** a Python venv: a venv's `pyvenv.cfg` records an absolute path back to the system Python
that created it and carries its own interpreter/`pip`/activation scripts, none of which is useful when the
runtime executing the code is an embedded RustPython/GraalPy interpreter. `pip install --target` produces the
same `site-packages`-shaped layout (including `.dist-info` metadata) without any of that baggage — plain
directories of `.py` files, which is all a `sys.path` insertion needs.

Because only pure wheels are allowed, bundled dependencies are just source with no per-platform variance —
unlike a compiled Lua rock, a bundled Python package needs none of the manifest's `entryPoints`-per-OS
machinery.

### External/shared packages (opt-in, LuaRocks-equivalent)

A `pythonPackagesDir` key in the manifest's `pluginConfig` (mirroring `luaRocksDir`), defaulting to something
like `~/.pact/python-packages`, adds an external directory to the search path in addition to the bundled one.
This exists for the same reason LuaRocks external support exists alongside JWT's vendoring: avoiding
duplicating a common/large dependency across every installed plugin that needs it.

### Open risk

Neither mechanism can *guarantee* a pure-Python package actually runs, only that it has no compiled extension.
RustPython's stdlib/language gaps mean a package with zero C dependencies can still fail to import for reasons
that have nothing to do with native code. This risk is materially smaller on the GraalPy/JVM side. It is not
present at all for Lua, where `mlua` vendors real Lua 5.4 — "pure Lua" and "will run correctly" are the same
guarantee; "pure Python" and "will run correctly under RustPython" are not.

## Technical details

No POC exists yet. Unlike proposals 002 and 003, which point at working branches, this proposal should not
move past Draft until a small prototype (following the JWT plugin's shape, or something simpler) validates:

* RustPython embedding in the Rust driver, following the same `executableType` dispatch pattern used for
  `"lua"` in `plugin_manager.rs` (e.g. `"python"`, gated behind its own optional cargo feature).
* GraalPy embedding in the JVM driver, following the same dispatch pattern in `PluginManager.kt`.
* The bundled `site-packages` + external `pythonPackagesDir` package resolution described above, on both
  drivers.
* Whether a handful of representative pure-Python packages (not just stdlib-only scripts) actually run
  correctly under RustPython, to get real data on the compatibility gap rather than relying on its own
  self-reported "not production ready" status.

## Benefits

* Validates that the script-plugin framework introduced for Lua generalises to a second language, rather than
  having accidentally been Lua-specific.
* Python is a far larger pool of potential plugin authors than Lua, directly serving proposal 002's original
  motivation of lowering the bar to write a plugin.
* The package-bundling pattern developed here (flat pure-wheel `site-packages`, vendored into the plugin
  directory) is itself reusable guidance for any future script language, not just Python.

## Issues with this approach

* **Compatibility gap on the Rust side.** RustPython is explicitly not production-ready and has real
  stdlib/language coverage gaps versus CPython. "Pure Python" is a weaker guarantee of "will actually run"
  than "pure Lua" was for `mlua`.
* **Behavioural inconsistency between drivers.** RustPython and GraalPy are two different Python
  implementations with different compatibility characteristics. A plugin that works under GraalPy is not
  guaranteed to work under RustPython, unlike Lua plugins, which run identical real-Lua-5.4 semantics on both
  drivers.
* **JVM dependency weight.** GraalPy/Truffle is a substantially heavier dependency than `luajava`'s small
  bundled native library.
* **No enforced pure-wheel restriction.** LuaRocks' `share/lua`/`lib/lua` split is structural — a driver
  simply never looks at `lib/lua`. Pip has no equivalent; the pure-wheel constraint is an install-time flag a
  plugin author must remember to use, and nothing currently stops a stray compiled `.so`/`.pyd` ending up in
  a bundled `site-packages` directory. A defensive scan-and-warn step is a possible mitigation but is not a
  structural fix.
* **The safer alternative (PyO3 + Jep) reintroduces the exact problem being solved for.** Both give real,
  consistent CPython semantics, but both require a system Python installation on every machine running the
  driver — the "System dependencies" issue proposal 002 already flagged as Python's weak point, and a step
  back from what Lua achieved.
