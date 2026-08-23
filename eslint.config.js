import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

export default tseslint.config(
  { ignores: ["dist/", "src-tauri/", "node_modules/"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    plugins: { "react-hooks": reactHooks },
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.browser },
    },
    rules: {
      // Hooks correctness is critical in this codebase (mega-effects, listeners)
      ...reactHooks.configs.recommended.rules,
      // Security-adjacent hygiene
      "no-eval": "error",
      "no-new-func": "error",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // Existing code relies on `any` in several view-layer spots; keep as
      // warnings so CI surfaces them without blocking the initial rollout.
      "@typescript-eslint/no-explicit-any": "warn",
    },
  },
);
