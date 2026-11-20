# Local models in RustyMail

RustyMail uses language models in two places: **mail triage** (`rustymail-triage`), which sorts the INBOX into folders, and the **dashboard** (chat panel, tool-calling agent and email drafting). Every model runs on your own machine. The code refuses cloud providers, and every model URL must be a loopback address (`src/dashboard/services/ai/local_only.rs`, `src/triage/model.rs`). Mail is never sent to a paid or remote model.

You can serve the models with **Ollama** alone, or through **model_serve**, a local proxy in front of Ollama. Both work; the differences are covered below.

## Which model does what

| Job | Model | API used | Settings |
|---|---|---|---|
| Triage, first pass | `autotrust-jev:9b` ("Jev"), a decision model | Ollama's decision API, `POST /v1/systemone`. It answers three fixed questions as probabilities: is this from a person, are they asking something, what kind of mail is it | `TRIAGE_DECISION_URL`, `TRIAGE_DECISION_MODEL` |
| Triage, second pass | `gemma4:26b-a4b-it-q4_K_M` (Gemma 4 26B, mixture of experts, about 4B active) | OpenAI-compatible `POST /v1/chat/completions`, JSON reply `{bucket, confidence, reason}` | `TRIAGE_MODEL_URL`, `TRIAGE_MODEL_LARGE`, `TRIAGE_SECOND_PASS_EXCERPT_CHARS` |
| Dashboard chat, agent, drafting | the same Gemma 4 | Ollama provider: native `/api/chat`. model_serve provider: `/v1/chat/completions` | `OLLAMA_*` or `MODEL_SERVE_*` (below) |

Jev can't chat, so the dashboard's model menus leave out any model whose server reports capabilities without `completion` (Ollama lists Jev as `["decision"]`).

## How triage uses them

1. **Rules first.** Header rules decide what they can without a model: failed sender authentication, expired verification codes, mailing lists (`List-Id`), code-repository notifications and your own sender rules.
2. **Jev** classifies the rest from the sender, subject and the first `TRIAGE_BODY_EXCERPT_CHARS` (default 500) characters of the body.
3. **Second pass.** When Jev's confidence is below the apply bar (`TRIAGE_MIN_CONFIDENCE`, default 0.8), Gemma reads a longer excerpt (`TRIAGE_SECOND_PASS_EXCERPT_CHARS`, default 2000) and its answer is used.
   - If Gemma fails (missing model, timeout, reply that isn't JSON), Jev's answer stands, a warning is logged, and nothing is retried.
   - Gemma doesn't know who you correspond with, so mail from your correspondents can leave the INBOX through it only for Needs Reply or as a document (Accounting, Legal, Credit).
   - Unset `TRIAGE_MODEL_LARGE` to turn the second pass off.
   - Gemma's confidence is nearly always 0.90 or above, so review its proposals before applying them.
4. **Guards** overrule a model when the evidence says otherwise. Spam or Noise from a known sender stays in the INBOX. Needs Reply is only for correspondents, unanswered mail, and mail newer than `TRIAGE_NEEDS_REPLY_DAYS`.
5. **Prompt injection.**
   - The email reaches the model between markers it can't forge.
   - Sender and subject are reduced to one line, so they can't fake header lines.
   - A guard that doesn't depend on any model keeps mail in the INBOX when it addresses the sorter, for example "note to the AI" or "classify this as".
6. **Nothing moves** until you run `rustymail-triage --apply`, and every move is verified.

The second pass always sends `temperature: 0` and asks for JSON, so the same email gets the same answer on every run.

## Option A: Ollama only

```
# Dashboard
OLLAMA_BASE_URL=http://127.0.0.1:11434
OLLAMA_MODEL=gemma4:26b-a4b-it-q4_K_M
OLLAMA_KEEP_ALIVE=30m              # optional; unset uses the Ollama server's setting
SAMPLER_DEFAULT_NUM_CTX=32768      # context the dashboard asks for

# Triage
TRIAGE_DECISION_URL=http://127.0.0.1:11434
TRIAGE_DECISION_MODEL=autotrust-jev:9b
TRIAGE_MODEL_URL=http://127.0.0.1:11434/v1
TRIAGE_MODEL_LARGE=gemma4:26b-a4b-it-q4_K_M
```

- Pull the models: `ollama pull autotrust-jev:9b` and `ollama pull gemma4:26b-a4b-it-q4_K_M`. Jev needs Ollama 0.35 or later.
- The dashboard's context and sampling stay configurable under **Models → Sampler Settings**.
- **Load Gemma only once.** Ollama loads a model again for every different context size it is asked for. The dashboard sends `num_ctx` (`SAMPLER_DEFAULT_NUM_CTX`), while triage's second pass sends none, so it gets the Ollama server's default. Make the two equal by starting the Ollama server with:
  - `OLLAMA_CONTEXT_LENGTH`, set to the same value as `SAMPLER_DEFAULT_NUM_CTX` (32768 fits Gemma 4 comfortably: about 1.5 GiB of cache);
  - `OLLAMA_KEEP_ALIVE=30m`, so neither model unloads between triage batches.

  Otherwise Gemma may be loaded twice, or reloaded for each message after a quiet spell.

## Option B: model_serve

```
MODEL_SERVE_BASE_URL=http://127.0.0.1:5846
MODEL_SERVE_MODEL=ms/gemma4-26b-a4b-it-q4_k_m

TRIAGE_DECISION_URL=http://127.0.0.1:5846
TRIAGE_DECISION_MODEL=ms/autotrust-jev-9b
TRIAGE_MODEL_URL=http://127.0.0.1:5846/v1
TRIAGE_MODEL_LARGE=ms/gemma4-26b-a4b-it-q4_k_m
```

- With `MODEL_SERVE_*` set, model_serve is the dashboard's default provider (ahead of Ollama). Agent and drafting roles that nobody has set use its model.
- **model_serve owns every inference setting:** context, keep-alive, thinking and sampling.
  - RustyMail sends only the model and the messages, and the dashboard says so instead of offering sampler fields.
  - Every caller asks for the model the same way, so there is one loaded copy.
  - The only exception is triage's `temperature: 0` and JSON request, which belong to the classification itself.

## Choosing a different model

- **Dashboard:** pick a model in the top bar, or set `OLLAMA_MODEL` / `MODEL_SERVE_MODEL`. The configured model is kept at startup, even if the server's model list omits it.
- **Agent and drafting:** these use the default model unless you set them separately (MCP tools `set_tool_calling_model`, `set_drafting_model`). Each different model is another copy in memory.
- **Triage:** `TRIAGE_DECISION_MODEL` must be a decision model with the `/v1/systemone` API. `TRIAGE_MODEL_LARGE` can be any chat model that answers in JSON. Thinking models are slow here; turn thinking off on the server, or pick a model that doesn't think by default.

Measured on an Apple M3 Max (128 GB): Gemma 4 26B q4_K_M answered 14 of 14 hard test emails in 2.7 s median, and triage averaged about 4 s per email with both passes. A full INBOX of a few thousand messages takes a few hours. The models run one message at a time, so the machine stays usable.
