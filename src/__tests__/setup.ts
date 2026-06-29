import "@testing-library/jest-dom";

// JSDOM doesn't implement window.matchMedia; provide a stub.
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
});

// Stub for clipboard API — jsdom doesn't implement it.
// Use writable: false to prevent tests from trying to redefine it.
if (!navigator.clipboard) {
  Object.defineProperty(navigator, "clipboard", {
    writable: false,
    value: {
      writeText: () => Promise.resolve(),
      readText: () => Promise.resolve(""),
    },
  });
}
