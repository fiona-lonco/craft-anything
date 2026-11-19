CRAFT ANYTHING - UNOFFICIAL VERSION (Fiona & Lonco Developer)
=============================================================

A modified version of Craft Anything (https://github.com/storytold/craft-anything). It is not
official: it is not affiliated with or endorsed by ArtCraft, Anthropic or any service it mentions.

WHAT IT ADDS
- Any AI model: keys for Claude, OpenAI, Gemini, DeepSeek, xAI, Groq, Mistral and more, your own
  address (OpenAI- or Anthropic-compatible) or models on your computer (Ollama, LM Studio).
- Picture and video AI (Settings -> Picture and video AI): OpenAI, Google Gemini, Google Imagen,
  xAI, Replicate, fal.ai, any web API or a model on your computer.
- Your own tools: turn a program or script into a tool for the AI. It always asks first.
- A fuller MCP form: environment variables, headers and whether it asks before each call.
- Go as an engine: ask for projects "in Go" (needs Go: https://go.dev/dl/).

HOW TO USE IT
1. Download CraftAnything-mod-...-windows-x64.exe and open it. Nothing else to install.
2. If Windows says "Windows protected your PC": click "More info", then "Run anyway". It shows
   because the program isn't signed with a paid certificate.
3. To build, it needs an AI. Any of these works (none is required in particular):
   - A coding agent you already have: Claude Code (the best tested), Codex, GitHub Copilot,
     Gemini CLI, OpenCode, Aider... The app finds it by itself.
   - Just an API key, nothing else to install: OpenAI, Claude, Gemini, DeepSeek, OpenRouter and
     more, in Settings -> AI.
   - A model on your computer, free and offline: Ollama or LM Studio. Building whole apps needs a
     large model; small ones give weaker results.

It's one file. On its first start it keeps its helper (craft-anything-cli.exe, which coding agents
run) in %APPDATA%\craft-anything\bin; its settings live in %APPDATA%\craft-anything too. It doesn't
update itself, so the official release can't replace it; get new versions where you got this one.
To uninstall, delete the .exe and the %APPDATA%\craft-anything folder.

LICENCE: MIT or Apache-2.0, like the original (the LICENSE-MIT, LICENSE-APACHE and NOTICE files in the repository).
