Check for hardcoded values from environment files

This command searches the source code for any hardcoded values that should be coming from environment variables instead.

Steps:

1. Extract all environment variable values from `.env.example` (ports, keys, hosts, etc.)
2. Search the `src/` and `frontend/` directories for these hardcoded values
3. Report any instances found with file path and line number
4. Flag this as a VIOLATION if any hardcoded values are found

Environment values to check:
- Ports: 9437, 9438, 9439
- API keys: test-rustymail-key-2024
- Hosts: 0.0.0.0, localhost
- Connection limits: 100, 60, 10, 30

The only acceptable places for these values:
- `.env` files
- `.env.example` files
- Documentation/README files
- Configuration schemas/types (NOT default values)

NOT acceptable:
- Fallback values in || operators
- Default values in code
- Hardcoded strings in source files
