# Elixir Integration

## Recommended shape

Keep the project split into three layers:

1. `spectre-core`
   Pure Rust logic. No Elixir concepts, no NIF macros, no BEAM-specific types.

2. `spectre-ffi`
   Thin boundary crate. This is where host adapters live:
   - C ABI for generic embedding
   - optional Rustler NIFs for Elixir

3. Elixir wrapper package/app
   A separate Elixir package that owns:
   - NIF loading
   - supervision
   - config (`model_dir`, `registry_mcr`)
   - JSON decode/encode at the edge
   - ergonomic Elixir APIs

This keeps the Rust engine reusable outside Elixir while still making Elixir integration straightforward.

## What to avoid

Do not move Elixir-specific logic into `spectre-core`.

Do not make the Rust core depend on BEAM terms or Elixir structs.

Do not treat the Elixir app as the source of truth for planner behavior. The planner should stay in Rust, with Elixir acting as a host.

## Best integration option

For an Elixir project, the best default is:

- keep using `spectre-core` for all planner logic
- keep using `spectre-ffi` for the boundary
- call it from Elixir through Rustler

That gives you:

- low call overhead
- one in-memory dispatcher/resource
- clean supervision on the Elixir side
- the ability to keep the same Rust crate usable from non-Elixir hosts

## When to prefer a Port instead

Use a Port instead of a NIF if you want stronger failure isolation than a NIF can provide, or if you expect long-running/blocking native work that you do not want inside the BEAM at all.

For this project, the planner path is small and already exposed through Dirty CPU NIFs in `spectre-ffi`, so Rustler is the better default.

## Suggested repository model

Use two repositories or two top-level packages:

- this repository: Rust workspace (`spectre-core`, `spectre-ffi`, CLI, training)
- a separate Elixir package, for example `spectre_kinetic_ex`

That Elixir package should depend on:

- `:rustler` if you want local compilation
- `:rustler_precompiled` if you want prebuilt binaries for users

This keeps release/distribution concerns out of the Rust workspace.

## Suggested Elixir package layout

```text
spectre_kinetic_ex/
  lib/
    spectre_kinetic.ex
    spectre_kinetic/native.ex
    spectre_kinetic/server.ex
  config/
    config.exs
  mix.exs
  native/
    spectre_ffi/
      Cargo.toml
      src/
```

There are two clean ways to populate `native/spectre_ffi`:

### Option A: thin Rustler crate that depends on this repo

Create a tiny Rustler crate inside the Elixir project and make it depend on `spectre-ffi` from Git or a path dependency.

Use this when:

- the Elixir package is the main product users will install
- you want normal Rustler ergonomics
- you may later publish precompiled binaries

This is usually the cleanest setup.

### Option B: reuse this repo directly during development

Point the Elixir project at this repository while developing locally, then extract a thin adapter crate later if you want a cleaner publishing story.

Use this when:

- you are iterating quickly
- you do not need Hex packaging yet

## Minimal Elixir wrapper shape

Your Elixir API should wrap the NIF resource instead of exposing raw JSON everywhere.

Example shape:

```elixir
defmodule SpectreKinetic do
  alias SpectreKinetic.Server

  def start_link(opts) do
    Server.start_link(opts)
  end

  def plan(server \\ Server, al_text) when is_binary(al_text) do
    Server.plan(server, al_text)
  end

  def add_action(server \\ Server, action) do
    Server.add_action(server, action)
  end

  def delete_action(server \\ Server, action_id) do
    Server.delete_action(server, action_id)
  end

  def reload_registry(server \\ Server, registry_path) do
    Server.reload_registry(server, registry_path)
  end
end
```

And keep the NIF module thin:

```elixir
defmodule SpectreKinetic.Native do
  use Rustler, otp_app: :spectre_kinetic_ex, crate: :spectre_ffi

  def open(_model_dir, _registry_mcr), do: :erlang.nif_error(:nif_not_loaded)
  def plan(_handle, _al_text), do: :erlang.nif_error(:nif_not_loaded)
  def plan_al(_handle, _al_text), do: :erlang.nif_error(:nif_not_loaded)
  def plan_json(_handle, _request_json), do: :erlang.nif_error(:nif_not_loaded)
  def add_action(_handle, _action_json), do: :erlang.nif_error(:nif_not_loaded)
  def delete_action(_handle, _action_id), do: :erlang.nif_error(:nif_not_loaded)
  def load_registry(_handle, _registry_mcr), do: :erlang.nif_error(:nif_not_loaded)
  def action_count(_handle), do: :erlang.nif_error(:nif_not_loaded)
  def version(), do: :erlang.nif_error(:nif_not_loaded)
end
```

Then put lifecycle and decoding in a server:

```elixir
defmodule SpectreKinetic.Server do
  use GenServer

  alias SpectreKinetic.Native

  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: Keyword.get(opts, :name, __MODULE__))
  end

  def plan(server, al_text), do: GenServer.call(server, {:plan, al_text})
  def add_action(server, action), do: GenServer.call(server, {:add_action, action})
  def delete_action(server, action_id), do: GenServer.call(server, {:delete_action, action_id})
  def reload_registry(server, registry_path), do: GenServer.call(server, {:reload_registry, registry_path})

  @impl true
  def init(opts) do
    model_dir = Keyword.fetch!(opts, :model_dir)
    registry_mcr = Keyword.fetch!(opts, :registry_mcr)
    handle = Native.open(model_dir, registry_mcr)
    {:ok, %{handle: handle}}
  end

  @impl true
  def handle_call({:plan, al_text}, _from, state) do
    {:reply, Jason.decode!(Native.plan(state.handle, al_text)), state}
  end

  def handle_call({:add_action, action}, _from, state) do
    {:reply, Native.add_action(state.handle, Jason.encode!(action)), state}
  end

  def handle_call({:delete_action, action_id}, _from, state) do
    {:reply, Native.delete_action(state.handle, action_id), state}
  end

  def handle_call({:reload_registry, registry_path}, _from, state) do
    {:reply, Native.load_registry(state.handle, registry_path), state}
  end
end
```

## Why a GenServer is useful

The Rustler resource already owns the dispatcher state, but wrapping it in a `GenServer` is still useful because it gives you:

- one supervised owner for config and initialization
- a clean place to hot-swap registries
- one place to normalize return values
- one place to convert JSON into Elixir maps/structs

## Practical recommendation

If you want something you can "drop into" Elixir cleanly:

1. Leave this Rust workspace mostly as-is.
2. Keep `spectre-core` independent.
3. Keep `spectre-ffi` as the native boundary crate.
4. Build a separate Elixir adapter package around the existing Rustler exports.
5. If distribution matters, add `rustler_precompiled` in the Elixir package later.

That is cleaner than pushing Elixir packaging concerns into this repository.
