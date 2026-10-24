// Provenance panel. Fetches ./provenance.json (relative, so GitHub Pages
// project sites and subpaths work) and degrades to an "unavailable" state when
// the file is absent or malformed. It never turns a missing file into a page
// error.
const state = document.querySelector("#provenance-state");
const body = document.querySelector("#provenance-body");
const unavailable = document.querySelector("#provenance-unavailable");

function value(text) {
  return text === null || text === undefined || text === "" ? "—" : String(text);
}

function link(href, text) {
  const anchor = document.createElement("a");
  anchor.href = href;
  anchor.textContent = text;
  anchor.rel = "noopener";
  return anchor;
}

function code(text) {
  const element = document.createElement("code");
  element.textContent = value(text);
  return element;
}

function row(list, label, content) {
  const term = document.createElement("dt");
  term.textContent = label;
  const description = document.createElement("dd");
  if (content instanceof Node) description.append(content);
  else description.textContent = value(content);
  list.append(term, description);
}

function shortDigest(reference) {
  if (!reference) return "—";
  const at = reference.lastIndexOf("@");
  return at === -1 ? reference : `${reference.slice(0, at)}@${reference.slice(at + 1, at + 19)}…`;
}

function hashRow(hash) {
  const wrapper = document.createElement("span");
  wrapper.className = "provenance-hash";
  const truncated = document.createElement("code");
  truncated.textContent = `${hash.slice(0, 20)}…`;
  truncated.title = hash;
  const copy = document.createElement("button");
  copy.type = "button";
  copy.textContent = "copy";
  copy.setAttribute("aria-label", "Copy full wasm SHA-256");
  copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(hash);
      copy.textContent = "copied";
      setTimeout(() => { copy.textContent = "copy"; }, 1500);
    } catch {
      copy.textContent = "unavailable";
    }
  });
  wrapper.append(truncated, copy);
  return wrapper;
}

function render(data) {
  const artifacts = data.artifacts ?? {};
  const wasm = artifacts.wasm;
  if (data.schema !== 1 || !wasm?.sha256) throw new Error("unsupported provenance schema");

  const list = document.createElement("dl");
  list.className = "provenance-grid";

  const source = data.source ?? {};
  if (source.commit) {
    const text = `${source.commit.slice(0, 12)}${source.dirty ? " (dirty)" : ""}`;
    const content = source.repo
      ? link(`${source.repo}/commit/${source.commit}`, text)
      : code(text);
    row(list, "commit", content);
  } else {
    row(list, "commit", "—");
  }
  row(list, "wasm sha256", hashRow(wasm.sha256));
  row(list, "wasm size", `${value(wasm.bytes)} bytes · gzip ${value(artifacts.wasm_gzip?.bytes)} bytes`);
  row(list, "rustc", code(data.toolchain?.rustc));
  row(list, "wasm-pack", code(data.toolchain?.wasm_pack));
  row(list, "wasm-bindgen", code(data.toolchain?.wasm_bindgen));
  row(list, "wasm-opt", code(data.toolchain?.wasm_opt));
  row(list, "rust image", code(shortDigest(data.images?.rust_base)));
  const rustImage = list.lastElementChild;
  if (rustImage) rustImage.title = value(data.images?.rust_base);
  row(list, "node image", code(shortDigest(data.images?.node_base)));
  const nodeImage = list.lastElementChild;
  if (nodeImage) nodeImage.title = value(data.images?.node_base);
  row(list, "platform", code(data.images?.platform));
  if (data.ci?.run_url) row(list, "ci run", link(data.ci.run_url, data.ci.workflow || "run"));
  else row(list, "ci run", "—");
  row(list, "verify", code(data.verify?.command));

  const links = document.createElement("p");
  links.className = "provenance-links";
  links.append(
    link("./provenance.json", "provenance.json"),
    document.createTextNode(" · "),
    link("./provenance.txt", "provenance.txt"),
    document.createTextNode(" · artifact identity is the wasm + js SHA-256; images are recorded, not claimed reproducible."),
  );

  body.replaceChildren(list, links);
  body.hidden = false;
  state.textContent = source.dirty ? "available (dirty build)" : "available";
}

let loaded = false;

async function load() {
  if (loaded) return;
  loaded = true;
  state.textContent = "loading…";
  try {
    const response = await fetch("./provenance.json", { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    render(await response.json());
  } catch {
    state.textContent = "unavailable";
    unavailable.hidden = false;
  }
}

// Lazy: do not request the file until the user opens the panel, so a local
// build without the generated file does not produce a 404 in the console.
const details = document.querySelector("#provenance-details");
details.addEventListener("toggle", () => {
  if (details.open) load();
});
