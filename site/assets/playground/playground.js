// writ playground. Every decision, record hash, verify and replay below is
// computed by the writ engine compiled to WebAssembly (crates/writ-wasm);
// this file is only UI. No framework, no network calls after load.
import init, * as W from "./writ_wasm.js";
import { POLICY_PRESETS, CALL_PRESETS, SESSIONS } from "./presets.js";

const $ = (id) => document.getElementById(id);
const J = (s) => JSON.parse(s);
const ESC = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ESC[c]);
const short = (h) => (h ? `${h.slice(0, 8)}…` : "");
const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
};

const state = {
  ready: false,
  compiled: null,
  errLine: null,
  hit: null, // { line, kind }
  ledger: [], // JSONL lines — the ledger "file"
  edited: new Set(),
  selected: null,
  verify: null,
  session: `pg-${Math.random().toString(16).slice(2, 8)}`,
  seq: 0,
  pendingAsk: null,
  samples: {},
};

// ---------------------------------------------------------------------------
// Policy editor

const src = $("ed-src");
const hl = $("ed-hl");
const gutter = $("ed-gutter");

const WHEN_TOKENS =
  /("(?:[^"\\]|\\.)*"?|'[^']*'?)|(\b(?:and|or|not|matches|startswith|endswith|contains|in)\b|==|!=)|(\b(?:tool|command|path|url\.host|query|server\.trust|server|agent|mode|hosts\.[\w-]+)\b)/g;
const PLAIN_TOKENS = /("(?:[^"\\]|\\.)*"?|'[^']*'?)|(\b(?:allow|deny|ask|redact)\b)|(\b(?:true|false|\d+(?:ms|s|m|h)?)\b)/g;

function tokens(text, re, classes) {
  let out = "";
  let last = 0;
  re.lastIndex = 0;
  for (let m; (m = re.exec(text)); ) {
    out += esc(text.slice(last, m.index));
    const g = classes.findIndex((_, i) => m[i + 1] !== undefined);
    const cls = typeof classes[g] === "function" ? classes[g](m[0]) : classes[g];
    out += cls ? `<span class="${cls}">${esc(m[0])}</span>` : esc(m[0]);
    last = m.index + m[0].length;
  }
  return out + esc(text.slice(last));
}

function highlightValue(key, v) {
  if (key === "id") return v.replace(/^(\s*)(.*)$/, (_, a, b) => `${esc(a)}<span class="t-id">${esc(b)}</span>`);
  if (key === "when") return tokens(v, WHEN_TOKENS, ["t-s", "t-op", "t-f"]);
  const verdictKey = key === "verdict" || key === "default";
  return tokens(v, PLAIN_TOKENS, ["t-s", (w) => (verdictKey ? `t-v-${w}` : ""), "t-n"]);
}

function highlightLine(line) {
  let code = line;
  let comment = "";
  let q = null;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (q) {
      if (c === "\\" && q === '"') i++;
      else if (c === q) q = null;
      continue;
    }
    if (c === '"' || c === "'") q = c;
    else if (c === "#" && (i === 0 || /\s/.test(line[i - 1]))) {
      code = line.slice(0, i);
      comment = line.slice(i);
      break;
    }
  }
  let out;
  const m = code.match(/^(\s*(?:-\s+)?)([A-Za-z_][\w.-]*)(\s*:)(.*)$/);
  if (m) {
    out = `${esc(m[1])}<span class="t-k">${esc(m[2])}</span>${esc(m[3])}${highlightValue(m[2], m[4])}`;
  } else {
    const li = code.match(/^(\s*-\s+)(.*)$/);
    out = li ? esc(li[1]) + highlightValue("", li[2]) : esc(code);
  }
  if (comment) out += `<span class="t-cm">${esc(comment)}</span>`;
  return out;
}

function renderEditor() {
  const lines = src.value.split("\n");
  let h = "";
  let g = "";
  lines.forEach((line, i) => {
    const n = i + 1;
    let cls = "l";
    let gcls = "";
    if (n === state.errLine) {
      cls += " err";
      gcls = "err";
    } else if (state.hit && n === state.hit.line) {
      cls += ` hit v-${state.hit.kind}`;
      gcls = "hit";
    }
    h += `<span class="${cls}">${highlightLine(line) || " "}</span>`;
    g += `<span class="${gcls}">${n}</span>`;
  });
  hl.innerHTML = h;
  gutter.innerHTML = g;
  syncScroll();
}

function syncScroll() {
  hl.scrollTop = src.scrollTop;
  hl.scrollLeft = src.scrollLeft;
  gutter.scrollTop = src.scrollTop;
}

function jumpToLine(line) {
  const lines = src.value.split("\n");
  const offset = lines.slice(0, line - 1).reduce((a, l) => a + l.length + 1, 0);
  src.focus();
  src.setSelectionRange(offset, offset + (lines[line - 1] || "").length);
  const lh = parseFloat(getComputedStyle(src).lineHeight) || 20;
  src.scrollTop = Math.max(0, (line - 4) * lh);
  syncScroll();
}

function compile() {
  const out = J(W.compile_policy(src.value));
  state.compiled = out;
  state.errLine = out.ok ? null : out.errors[0]?.line ?? null;
  const box = $("compile-status");
  if (out.ok) {
    box.className = "compile ok";
    box.innerHTML = `compiled · ${out.rules.length} rule${out.rules.length === 1 ? "" : "s"} · default <b class="chip sm ${out.default}">${out.default}</b> · version ${out.version}`;
  } else {
    const e = out.errors[0];
    box.className = "compile bad";
    const where = e.line
      ? `<button type="button" class="jump" data-line="${e.line}">writ.yaml:${e.line}${e.column ? `:${e.column}` : ""}</button>`
      : "writ.yaml";
    box.innerHTML = `does not compile · ${where} <span class="msg">— ${esc(e.message)}</span>`;
  }
  renderRules();
  renderEditor();
  $("run-btn").disabled = !state.ready;
}

function renderRules() {
  const el = $("rule-list");
  const c = state.compiled;
  if (!c || !c.ok) {
    el.innerHTML = "";
    return;
  }
  const hitId = state.hitRule;
  el.innerHTML =
    c.rules
      .map(
        (r) =>
          `<button type="button" class="rule-chip${r.id === hitId ? " hit" : ""}" data-line="${r.line ?? ""}" title="${esc(r.reason || r.verdict)}${r.line ? ` — writ.yaml:${r.line}` : ""}"><span class="sw ${r.verdict}" aria-hidden="true"></span>${esc(r.id)}<span class="sr"> (${r.verdict})</span></button>`,
      )
      .join("") +
    `<span class="rule-chip default${hitId === "default" ? " hit" : ""}"><span class="sw ${c.default}" aria-hidden="true"></span>default: ${c.default}</span>`;
}

let compileTimer = 0;
src.addEventListener("input", () => {
  state.hit = null;
  state.hitRule = null;
  renderEditor();
  clearTimeout(compileTimer);
  compileTimer = setTimeout(() => {
    compile();
    store.set("writ-playground-policy", src.value);
    markCustomPolicy();
  }, 120);
});
src.addEventListener("scroll", syncScroll);
src.addEventListener("focus", () => $("editor").classList.add("focus"));
src.addEventListener("blur", () => $("editor").classList.remove("focus"));
let tabReleased = false;
src.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    tabReleased = true;
    return;
  }
  if (e.key === "Tab" && !tabReleased && !e.shiftKey && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    const { selectionStart: a, selectionEnd: b, value } = src;
    src.value = value.slice(0, a) + "  " + value.slice(b);
    src.setSelectionRange(a + 2, a + 2);
    src.dispatchEvent(new Event("input"));
    return;
  }
  tabReleased = false;
});
document.addEventListener("click", (e) => {
  const t = e.target.closest("[data-line]");
  if (t && t.dataset.line) jumpToLine(Number(t.dataset.line));
});

function policySource(p) {
  return p.kind === "pack" ? W.pack_to_policy(p.source) : p.source;
}

function fillPolicyPresets() {
  const sel = $("policy-preset");
  const groups = { example: "examples/", pack: "packs/" };
  sel.innerHTML =
    Object.entries(groups)
      .map(
        ([kind, label]) =>
          `<optgroup label="${label}">${POLICY_PRESETS.filter((p) => p.kind === kind)
            .map((p) => `<option value="${esc(p.id)}">${esc(p.id.replace(/\/pack\.yaml$/, ""))} — ${esc(p.label)}</option>`)
            .join("")}</optgroup>`,
      )
      .join("") + `<option value="custom" hidden>Your edits</option>`;
  sel.addEventListener("change", () => {
    const p = POLICY_PRESETS.find((x) => x.id === sel.value);
    if (!p) return;
    src.value = policySource(p);
    src.scrollTop = 0;
    state.hit = null;
    state.hitRule = null;
    store.set("writ-playground-policy", src.value);
    compile();
  });
}

function markCustomPolicy() {
  const sel = $("policy-preset");
  const p = POLICY_PRESETS.find((x) => policySource(x) === src.value);
  sel.value = p ? p.id : "custom";
}

$("policy-download").addEventListener("click", () => download("writ.yaml", src.value, "text/yaml"));

// ---------------------------------------------------------------------------
// Agent call simulator

const f = {
  agent: $("f-agent"),
  tool: $("f-tool"),
  server: $("f-server"),
  trust: $("f-trust"),
  args: $("f-args"),
  output: $("f-output"),
};

function fillCallPresets() {
  const sel = $("call-preset");
  const groups = [...new Set(CALL_PRESETS.map((c) => c.group))];
  sel.innerHTML =
    groups
      .map(
        (g) =>
          `<optgroup label="${esc(g)}">${CALL_PRESETS.filter((c) => c.group === g)
            .map((c) => `<option value="${c.id}">${esc(g)}: ${esc(c.label)}</option>`)
            .join("")}</optgroup>`,
      )
      .join("") + `<option value="custom">Custom call…</option>`;
  sel.addEventListener("change", () => {
    const c = CALL_PRESETS.find((x) => x.id === sel.value);
    if (c) loadCall(c);
    else f.tool.focus();
  });
  for (const el of Object.values(f)) el.addEventListener("input", () => (sel.value = "custom"));
}

function loadCall(c) {
  $("call-preset").value = c.id;
  f.agent.value = c.call.agent;
  f.tool.value = c.call.tool;
  f.server.value = c.call.server || "";
  f.trust.value = c.call.trust || "";
  f.args.value = JSON.stringify(c.call.args, null, 2);
  f.output.value = c.output || "";
  validateArgs();
}

function validateArgs() {
  const err = $("args-err");
  try {
    const v = JSON.parse(f.args.value.trim() || "{}");
    if (v === null || typeof v !== "object" || Array.isArray(v)) throw new Error("arguments must be a JSON object");
    err.textContent = "";
    f.args.removeAttribute("aria-invalid");
    return v;
  } catch (e) {
    err.textContent = `Arguments are not valid JSON: ${e.message}`;
    f.args.setAttribute("aria-invalid", "true");
    return null;
  }
}
f.args.addEventListener("input", validateArgs);

const summarize = (args) => {
  const s = JSON.stringify(args);
  return s.length > 140 ? `${s.slice(0, 139)}…` : s;
};

function run() {
  if (!state.ready) return;
  if (state.pendingAsk) answerAsk(null); // an unanswered ask fails closed
  const out = $("verdict");
  if (!state.compiled?.ok) {
    const e = state.compiled?.errors?.[0];
    out.innerHTML = `<div class="err-box">The policy does not compile, so writ would refuse to start: ${e?.line ? `writ.yaml:${e.line} — ` : ""}${esc(e?.message)}</div>`;
    return;
  }
  const args = validateArgs();
  if (!args) {
    f.args.focus();
    return;
  }
  if (!f.tool.value.trim()) {
    f.tool.focus();
    return;
  }
  state.seq += 1;
  const call = {
    agent: f.agent.value,
    tool: f.tool.value.trim(),
    args,
    server: f.server.value.trim() || null,
    trust: f.trust.value || null,
    call_id: `toolu_${String(state.seq).padStart(4, "0")}`,
    session_id: state.session,
  };
  const now = Date.now();
  const ev = J(W.evaluate(src.value, JSON.stringify(call), now));
  if (!ev.ok) {
    out.innerHTML = `<div class="err-box">${esc(ev.error)}</div>`;
    return;
  }
  const rule = state.compiled.rules.find((r) => r.id === ev.verdict.rule_id);
  state.hit = rule?.line ? { line: rule.line, kind: ev.kind } : null;
  state.hitRule = ev.verdict.rule_id || null;
  renderEditor();
  renderRules();

  const ctx = { ev, output: f.output.value, now, agentTool: call.tool, agent: call.agent };
  if (ev.kind === "ask") {
    state.pendingAsk = ctx;
    renderVerdict(ctx, null);
    out.querySelector(".ask-actions .allow")?.focus();
  } else {
    const rec = record(ev, null, ev.kind !== "deny", ctx.output, now);
    renderVerdict(ctx, rec);
  }
}

$("call-form").addEventListener("submit", (e) => {
  e.preventDefault();
  run();
});
document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    run();
  }
});

/** Append a decision (and, if dispatched, an execution) record. */
function record(ev, approver, dispatched, output, now) {
  const approverJson = approver ? JSON.stringify(approver) : "";
  const d = J(W.ledger_record_decision(state.ledger.at(-1) ?? "", JSON.stringify(ev.call), JSON.stringify(ev.verdict), approverJson, now));
  if (!d.ok) throw new Error(d.error);
  state.ledger.push(JSON.stringify(d.record));
  const res = { decision: d.record.index, execution: null };
  if (dispatched) {
    const e = J(W.ledger_record_execution(state.ledger.at(-1), JSON.stringify(d.record), "host", 0, output ?? "", now + 180));
    if (!e.ok) throw new Error(e.error);
    state.ledger.push(JSON.stringify(e.record));
    res.execution = e.record.index;
  }
  const hadVerified = state.verify !== null;
  state.verify = null;
  state.fresh = new Set([res.decision, res.execution].filter((x) => x !== null));
  if (hadVerified) showVerify(); // keep the verify line truthful for the grown chain
  else renderLedger();
  return res;
}

function answerAsk(allow) {
  const ctx = state.pendingAsk;
  if (!ctx) return;
  state.pendingAsk = null;
  let approver;
  if (allow === null) {
    const t = ctx.ev.verdict.timeout_ms;
    approver = { kind: "outofband", id: `fail-closed(timeout=${t == null ? "None" : `Some(${t})`})` };
  } else {
    approver = J(W.playground_approver());
  }
  const rec = record(ctx.ev, approver, allow === true, ctx.output, Date.now());
  ctx.answer = { allow: allow === true, approver, unanswered: allow === null };
  renderVerdict(ctx, rec);
}

function sawRows(ev) {
  const c = ev.ctx;
  const rows = [
    ["tool", c.tool],
    ["command", c.command],
    ["path", c.path],
    ["url.host", c.url_host],
    ["query", c.query],
    ["server", c.server],
    ["server.trust", c.trust],
    ["agent", c.agent],
    ["mode", { mcp: "mcp", processwrap: "processwrap", sdkhook: "sdkhook" }[c.mode] || c.mode],
  ];
  return rows
    .map(([k, v]) => `<dt>${k}</dt><dd class="${v == null ? "none" : ""}">${v == null ? "—" : esc(v)}</dd>`)
    .join("");
}

function markRedactions(s) {
  return esc(s).replaceAll("[redacted-by-writ]", '<span class="mark">[redacted-by-writ]</span>');
}

function renderVerdict(ctx, rec) {
  const { ev } = ctx;
  const v = ev.verdict;
  const k = ev.kind;
  const loc = v.location ? `<button type="button" class="loc" data-line="${v.location.split(":")[1]}">${esc(v.location)}</button>` : "";
  const recNote = rec
    ? `<span class="rec">ledger #${rec.decision}${rec.execution !== null ? ` + #${rec.execution}` : ""}</span>`
    : `<span class="rec">waiting for a human</span>`;
  const mapped =
    ctx.agentTool !== ev.call.tool || ev.call.server
      ? `<p class="map-note">${esc(agentName(ctx.agent))} sent <code>${esc(ctx.agentTool)}</code>; writ evaluates it as <code>${esc(ev.call.tool)}</code>${ev.call.server ? ` on server <code>${esc(ev.call.server.name)}</code>` : ""}.</p>`
      : "";
  let body = "";
  if (k === "deny") {
    body += `<div class="v-reason"><span class="lbl">Returned to the agent</span>${esc(v.reason)}</div>`;
  } else if (k === "allow") {
    body += `<div class="v-reason"><span class="lbl">Dispatched</span>${v.rule_id === "default" ? "No rule matched and the policy default is <code>allow</code>." : `Allowed by rule <code>${esc(v.rule_id)}</code>.`}</div>`;
  } else if (k === "redact") {
    const m = J(W.mask_output(JSON.stringify(v.patterns), ctx.output || ""));
    body += `<div class="v-reason"><span class="lbl">Dispatched, result masked</span>The call runs; ${m.ok ? `${m.matches} match${m.matches === 1 ? "" : "es"} of <code>${v.patterns.length}</code> pattern${v.patterns.length === 1 ? "" : "s"}` : "the patterns"} are masked before the result re-enters the model's context. The ledger keeps the hash of the original.</div>`;
    body += m.ok
      ? `<div class="redact-cols"><div><span class="v-sub">Tool returned</span><pre>${esc(ctx.output || "(empty — add a tool result above)")}</pre></div><div><span class="v-sub">The model sees</span><pre>${markRedactions(m.masked || "")}</pre></div></div>`
      : `<div class="err-box">${esc(m.error)}</div>`;
  } else if (k === "ask") {
    const a = ctx.answer;
    body += `<div class="ask-box" role="group" aria-label="Approval prompt">
      <div><span class="warn">⚠ writ asks:</span> ${esc(ev.call.tool)} ${esc(summarize(ev.call.args))}</div>
      ${v.diff.split("\n").map((l) => `<div class="indent">${esc(l)}</div>`).join("")}
      <div class="indent">rule: ${esc(v.rule_id)}${v.timeout_ms ? ` · timeout ${fmtMs(v.timeout_ms)} (fails closed)` : ""}</div>
      ${v.irreversible ? `<div class="indent irr">this action is marked IRREVERSIBLE</div>` : ""}
      ${
        a
          ? `<div class="indent outcome">${
              a.allow
                ? `<b class="allow">allowed</b> by ${a.approver.kind}:${esc(a.approver.id)} → dispatched`
                : a.unanswered
                  ? `<b class="deny">no answer</b> → fail closed, recorded with approver ${esc(a.approver.id)}`
                  : `<b class="deny">denied</b> by ${a.approver.kind}:${esc(a.approver.id)} → nothing dispatched`
            }</div>`
          : `<div class="ask-actions"><button type="button" class="btn allow" data-ask="allow"><u>a</u>llow once</button><button type="button" class="btn deny" data-ask="deny"><u>d</u>eny</button><span class="hint">keys: a / d</span></div>`
      }
    </div>`;
  }
  body += mapped;
  body += `<details class="more"><summary>What writ evaluated (<code>ToolCallContext</code>)</summary><dl class="saw">${sawRows(ev)}</dl></details>`;
  $("verdict").innerHTML = `<article class="verdict k-${k}">
    <div class="v-top"><span class="chip ${k}">${k}</span><code>${esc(v.rule_id ?? "default")}</code>${loc}${recNote}</div>
    <div class="v-body">${body}</div></article>`;
}

$("verdict").addEventListener("click", (e) => {
  const b = e.target.closest("[data-ask]");
  if (b) answerAsk(b.dataset.ask === "allow");
});
$("verdict").addEventListener("keydown", (e) => {
  if (!state.pendingAsk || e.ctrlKey || e.metaKey || e.altKey) return;
  if (e.key === "a") answerAsk(true);
  else if (e.key === "d") answerAsk(false);
});

const agentName = (a) => ({ "claude-code": "Claude Code", mcp: "The MCP client", langgraph: "LangGraph", "openai-agents": "The OpenAI Agents SDK" })[a] || a;
const fmtMs = (ms) => (ms % 3_600_000 === 0 ? `${ms / 3_600_000}h` : ms % 60_000 === 0 ? `${ms / 60_000}m` : ms % 1000 === 0 ? `${ms / 1000}s` : `${ms}ms`);

// ---------------------------------------------------------------------------
// Ledger

function parsed() {
  return state.ledger.map((l) => {
    try {
      return JSON.parse(l);
    } catch {
      return null;
    }
  });
}

function renderLedger() {
  const recs = parsed();
  const chain = $("chain");
  const n = recs.length;
  for (const id of ["ledger-download", "ledger-reset"]) $(id).disabled = n === 0;
  if (state.selected !== null && state.selected >= n) state.selected = n ? n - 1 : null;
  $("tamper-btn").disabled = state.selected === null;
  $("tamper-idx").textContent = state.selected === null ? "–" : recs[state.selected]?.index ?? state.selected;
  if (!n) {
    chain.innerHTML = `<li class="empty">No records yet. Run a call: every decision lands here, denials included, each one hashed over the one before it.</li>`;
    $("record-detail").hidden = true;
    return;
  }
  const vr = state.verify;
  const fresh = state.fresh || new Set();
  chain.innerHTML = recs
    .map((r, i) => {
      const prev = recs[i - 1];
      const linkOk = i === 0 ? r?.prev_hash === "0".repeat(64) : r && prev && r.prev_hash === prev.record_hash;
      const link = i === 0 ? "" : `<span class="link ${linkOk ? "ok" : "broken"}" aria-hidden="true"></span>`;
      let cls = "block";
      let flag = "";
      if (state.edited.has(i)) {
        cls += " edited";
        flag = `<span class="flag">edited</span>`;
      }
      if (vr) {
        if (vr.intact || i < vr.records) cls += " v-ok";
        else if (i === vr.records) {
          cls += " v-broken";
          flag = `<span class="flag">breaks here</span>`;
        } else cls += " v-after";
      }
      if (r && fresh.has(r.index) && !vr) cls += " fresh";
      let what;
      if (!r) what = `<span class="b-tool">unparseable</span>`;
      else if (r.kind === "decision") {
        const k = r.verdict?.kind || "?";
        what = `<span class="b-tool" title="${esc(r.call?.tool)}"><span class="chip sm ${k}">${k}</span> ${esc(r.call?.tool)}</span>`;
      } else {
        what = `<span class="b-tool">↳ executed #${r.decision_index} · exit ${r.exit_status}</span>`;
      }
      const label = r ? `Record ${r.index}, ${r.kind}${r.verdict ? `, ${r.verdict.kind}` : ""}` : `Record at position ${i}, unparseable`;
      return `<li class="blk">${link}<button type="button" class="${cls}" data-pos="${i}" aria-pressed="${state.selected === i}" aria-label="${esc(label)}">${flag}
        <span class="b-top"><span class="b-idx">#${r ? r.index : "?"}</span><span class="b-kind">${r ? r.kind : ""}${r?.approver ? " · approver" : ""}</span></span>
        ${what}
        <span class="b-hash"><b>prev</b> ${short(r?.prev_hash)}</span>
        <span class="b-hash"><b>hash</b> ${short(r?.record_hash)}</span></button></li>`;
    })
    .join("");
  state.fresh = null;
  const last = chain.lastElementChild;
  if (fresh.size && last) chain.parentElement.scrollLeft = chain.parentElement.scrollWidth;
  renderDetail();
}

function jsonHtml(obj) {
  return esc(JSON.stringify(obj, null, 2)).replace(
    /(&quot;(?:[^&]|&(?!quot;))*?&quot;)(\s*:)?|\b(-?\d+(?:\.\d+)?|true|false|null)\b/g,
    (m, str, colon, lit) => {
      if (str) {
        if (colon) return `<span class="jk">${str}</span>${colon}`;
        return /^&quot;[0-9a-f]{64}&quot;$/.test(str) ? `<span class="jh">${str}</span>` : `<span class="js">${str}</span>`;
      }
      return `<span class="jn">${lit}</span>`;
    },
  );
}

function renderDetail() {
  const box = $("record-detail");
  if (state.selected === null) {
    box.hidden = true;
    return;
  }
  const line = state.ledger[state.selected];
  let rec = null;
  try {
    rec = JSON.parse(line);
  } catch {
    /* shown raw */
  }
  box.hidden = false;
  $("rd-title").textContent = rec ? `record #${rec.index} · ${rec.kind}` : "unparseable record";
  $("rd-note").textContent = "record_hash = SHA-256 over every other field (writ_core::LedgerRecord::compute_hash)";
  $("rd-json").innerHTML = rec ? jsonHtml(rec) : esc(line);
}

$("chain").addEventListener("click", (e) => {
  const b = e.target.closest("[data-pos]");
  if (!b) return;
  const pos = Number(b.dataset.pos);
  state.selected = state.selected === pos ? null : pos;
  renderLedger();
  document.querySelector(`[data-pos="${pos}"]`)?.focus();
});

function showVerify() {
  const out = $("verify-out");
  if (!state.ledger.length) {
    out.innerHTML = `<span class="prompt">$</span> writ verify <span class="dim">— no ledger yet; run a call first</span>`;
    return;
  }
  const r = J(W.ledger_verify(state.ledger.join("\n")));
  state.verify = r;
  out.innerHTML = `<span class="prompt">$</span> writ verify<br>${
    r.intact ? `<span class="ok">${esc(r.message)}</span>` : `<span class="bad">${esc(r.message)}</span><span class="why">${esc(r.why || "")}</span>`
  }${
    r.intact && state.edited.size
      ? `<span class="why">The chain still verifies: rewriting the newest record together with its own hash leaves nothing after it to disagree. Anchoring the tip externally (on the roadmap) is what closes that gap.</span>`
      : ""
  }`;
  renderLedger();
}

$("verify-btn").addEventListener("click", showVerify);

$("tamper-btn").addEventListener("click", () => {
  const pos = state.selected;
  if (pos === null) return;
  const mode = $("tamper-mode").value;
  const t = J(W.ledger_tamper(state.ledger.join("\n"), pos, mode));
  if (!t.ok) {
    $("verify-out").innerHTML = `<span class="bad">${esc(t.error)}</span>`;
    return;
  }
  state.ledger = t.jsonl.split("\n").filter((l) => l.trim());
  if (mode === "delete") {
    state.edited = new Set([...state.edited].filter((i) => i !== pos).map((i) => (i > pos ? i - 1 : i)));
    state.selected = null;
  } else state.edited.add(pos);
  state.verify = null;
  $("verify-out").innerHTML = `<span class="prompt">#</span> <span class="dim">an attacker ${esc(t.what)}. Now run</span> writ verify`;
  renderLedger();
  $("verify-btn").focus();
});

$("ledger-reset").addEventListener("click", () => {
  state.ledger = [];
  state.edited = new Set();
  state.selected = null;
  state.verify = null;
  $("verify-out").innerHTML = `<span class="prompt">$</span> writ verify <span class="dim">— run a few calls first</span>`;
  renderLedger();
  refreshReplaySources();
});

$("ledger-download").addEventListener("click", () => download("ledger.jsonl", `${state.ledger.join("\n")}\n`, "application/x-ndjson"));

function download(name, text, type) {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = Object.assign(document.createElement("a"), { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// ---------------------------------------------------------------------------
// Replay

function buildSample(s) {
  const lines = [];
  const base = Date.UTC(2026, 8, 18, 16, 2, 0);
  s.calls.forEach((id, i) => {
    const p = CALL_PRESETS.find((c) => c.id === id);
    const t = base + i * 37_000;
    const call = { ...p.call, session_id: s.session_id, call_id: `${s.id}-${String(i + 1).padStart(2, "0")}` };
    const ev = J(W.evaluate(s.recorded_under, JSON.stringify(call), t));
    const appr = ev.kind === "ask" ? W.playground_approver() : "";
    const d = J(W.ledger_record_decision(lines.at(-1) ?? "", JSON.stringify(ev.call), JSON.stringify(ev.verdict), appr, t));
    lines.push(JSON.stringify(d.record));
    if (ev.kind !== "deny") {
      const e = J(W.ledger_record_execution(lines.at(-1), JSON.stringify(d.record), "host", 0, p.output ?? "", t + 1_200));
      lines.push(JSON.stringify(e.record));
    }
  });
  return lines;
}

function refreshReplaySources() {
  const sel = $("rp-source");
  const cur = sel.value;
  const decisions = parsed().filter((r) => r?.kind === "decision").length;
  sel.innerHTML =
    SESSIONS.map((s) => `<option value="sample:${s.id}">Sample: ${esc(s.label)}</option>`).join("") +
    `<option value="playground"${decisions ? "" : " disabled"}>This playground's ledger (${decisions} call${decisions === 1 ? "" : "s"})</option>` +
    `<option value="paste">Paste a ledger.jsonl…</option>`;
  if ([...sel.options].some((o) => o.value === cur && !o.disabled)) sel.value = cur;
  $("rp-paste").hidden = sel.value !== "paste";
}

function fillCandidates() {
  const sel = $("rp-candidate");
  sel.innerHTML =
    `<option value="editor">The policy in the editor</option>` +
    POLICY_PRESETS.map((p) => `<option value="${esc(p.id)}">${esc(p.id)}</option>`).join("");
}

function pastedSessions() {
  const ids = new Set();
  for (const l of $("rp-jsonl").value.split("\n")) {
    try {
      const r = JSON.parse(l);
      if (r?.session_id) ids.add(r.session_id);
    } catch {
      /* ignore */
    }
  }
  const sel = $("rp-session");
  const cur = sel.value;
  sel.innerHTML = ids.size ? [...ids].map((s) => `<option>${esc(s)}</option>`).join("") : `<option value="">(no records found)</option>`;
  if (ids.has(cur)) sel.value = cur;
}

$("rp-source").addEventListener("change", () => {
  $("rp-paste").hidden = $("rp-source").value !== "paste";
});
$("rp-jsonl").addEventListener("input", pastedSessions);

function runReplay() {
  const source = $("rp-source").value;
  let jsonl;
  let session;
  if (source.startsWith("sample:")) {
    const s = SESSIONS.find((x) => `sample:${x.id}` === source);
    state.samples[s.id] ??= buildSample(s);
    jsonl = state.samples[s.id].join("\n");
    session = s.session_id;
  } else if (source === "playground") {
    jsonl = state.ledger.join("\n");
    session = state.session;
  } else {
    jsonl = $("rp-jsonl").value;
    session = $("rp-session").value;
  }
  const cand = $("rp-candidate").value;
  const p = POLICY_PRESETS.find((x) => x.id === cand);
  const candidate = p ? policySource(p) : src.value;
  const out = J(W.replay(jsonl, session, candidate));
  const box = $("rp-out");
  if (!out.ok) {
    box.innerHTML = `<div class="err-box" style="margin-top:14px">${esc(out.error)}</div>`;
    return;
  }
  const blocked = out.rows.filter((r) => r.newly_blocked).length;
  box.innerHTML = `<div class="rp-summary"><span class="dim">$ writ replay ${esc(session)} --candidate ${esc(p ? p.id : "writ.yaml")}</span><br>${esc(out.summary).replace(
    /(\d+) previously-allowed call\(s\) would now be blocked/,
    (m, n) => `<span class="${Number(n) ? "hl" : "okc"}">${m}</span>`,
  )}</div>
  <div class="table-wrap"><table class="rp"><thead><tr><th scope="col">#</th><th scope="col">Recorded call</th><th scope="col">Recorded</th><th scope="col">Candidate</th><th scope="col">Rule</th></tr></thead><tbody>${out.rows
    .map(
      (r) => `<tr class="${r.newly_blocked ? "blocked" : ""}${r.changed ? " changed" : ""}">
      <td class="idx">${r.index}</td>
      <td class="call"><span class="tool">${esc(r.tool)}</span> <span class="args">${esc(summarize(r.args))}</span></td>
      <td><span class="chip sm ${r.was}">${r.was}</span>${r.executed ? ` <span class="ran">ran</span>` : ""}</td>
      <td><span class="chip sm ${r.now}">${r.now}</span>${r.newly_blocked ? ` <span class="tag">would now be blocked</span>` : ""}</td>
      <td class="rule">${esc(r.now_rule ?? "")}</td></tr>`,
    )
    .join("")}</tbody></table></div>`;
  box.dataset.blocked = String(blocked);
}

$("rp-run").addEventListener("click", runReplay);

// ---------------------------------------------------------------------------
// Copy buttons

document.querySelectorAll("[data-copy]").forEach((b) =>
  b.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(b.dataset.copy);
      b.textContent = "copied";
      b.classList.add("done");
      setTimeout(() => {
        b.textContent = "copy";
        b.classList.remove("done");
      }, 1400);
    } catch {
      b.textContent = "select + copy";
    }
  }),
);

// ---------------------------------------------------------------------------
// Boot

async function boot() {
  const status = $("engine-status");
  try {
    await init({ module_or_path: new URL("writ_wasm_bg.wasm", import.meta.url) });
  } catch (e) {
    status.classList.add("failed");
    status.lastElementChild.textContent = `could not load the engine (${e.message}); this page needs WebAssembly`;
    return;
  }
  state.ready = true;
  const v = W.version();
  status.classList.add("ready");
  status.lastElementChild.textContent = `writ-wasm ${v} · engine ready · running in this tab`;
  $("engine-version").textContent = `Engine: writ-wasm ${v}.`;

  fillPolicyPresets();
  fillCallPresets();
  fillCandidates();
  const saved = store.get("writ-playground-policy");
  const first = POLICY_PRESETS[0];
  src.value = saved && saved.trim() ? saved : policySource(first);
  markCustomPolicy();
  compile();
  loadCall(CALL_PRESETS[0]);
  refreshReplaySources();
  pastedSessions();
  renderLedger();
  $("rp-run").disabled = false;
  new MutationObserver(refreshReplaySources).observe($("chain"), { childList: true });

  if (location.hash.startsWith("#demo")) demo(location.hash.split("=")[1]);
}

/** `playground.html#demo`: a few calls, one tamper, one verify, one replay. */
function demo(last) {
  src.value = policySource(POLICY_PRESETS[0]);
  compile();
  const call = (id) => {
    loadCall(CALL_PRESETS.find((c) => c.id === id));
    run();
  };
  ["cc-bash-rm", "mcp-postgres-select", "cc-webfetch-evil"].forEach(call);
  call("mcp-postgres-drop");
  answerAsk(true);
  call(last === "ask" ? "cc-bash-test" : "mcp-postgres-select");
  if (last === "ask") call("mcp-postgres-drop");
  state.selected = 1;
  renderLedger();
  $("tamper-mode").value = "edit";
  $("tamper-btn").click();
  showVerify();
  state.selected = 1;
  renderLedger();
  $("chain").parentElement.scrollLeft = 0;
  $("rp-candidate").value = "examples/writ.yaml";
  runReplay();
  window.scrollTo(0, 0);
}

boot();
