Check for and automatically fix hardcoded values from environment files

This command searches the source code for any hardcoded values that should be coming from environment variables, then automatically fixes them.

Steps:

1. Extract all environment variable values from `.env.example` (ports, keys, hosts, etc.)
2. Search the `src/` and `frontend/` directories for these hardcoded values
3. **AUTOMATICALLY FIX** each violation by:
   - Removing hardcoded default values (use `panic!` or return `Err` if env var missing)
   - Removing fallback values in `|| operators` (remove the `|| "default"` part)
   - Replacing hardcoded strings with proper env var reads
   - Adding clear error messages when env vars are required but missing
4. Report all fixes made with file path and line number
5. Verify no hardcoded values remain after fixes
6. Remember that variables used in the code must be defined in the .env.example file. For example, according to the .env file, it's "OLLAMA_BASE_URL", not "OLLAMA_API_BASE". Don't just make it up off the top of your head -- find the correct variable in .env.example and use that same variable name in the code so they always match.

Environment values to check and fix:
- Ports: 9437, 9438, 9439
- API keys: test-rustymail-key-2024
- Hosts: 0.0.0.0, localhost
- Connection limits: 100, 60, 10, 30

The only acceptable places for these values:
- `.env` files
- `.env.example` files
- Documentation/README files
- Configuration schemas/types (NOT default values)

NOT acceptable (MUST BE FIXED):
- Fallback values in || operators → REMOVE the fallback
- Default values in code → REMOVE and require env var
- Hardcoded strings in source files → REPLACE with env var reads

After fixing, rebuild and test to ensure the application properly requires environment variables.
