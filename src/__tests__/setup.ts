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

// JSDOM doesn't implement window.Notification; provide a stub.
Object.defineProperty(window, "Notification", {
  writable: true,
  value: {
    permission: "default",
    requestPermission: () => Promise.resolve("default"),
  },
});
