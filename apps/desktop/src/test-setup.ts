import { cleanup, configure } from "@testing-library/react";
import { afterEach, vi } from "vitest";
// Node 26 exposes a global storage getter without a backing file. Keep these
// synthetic tests isolated from both that getter and any real browser profile.
const values = new Map<string, string>();
const storage: Storage = {
  get length() {
    return values.size;
  },
  clear: () => values.clear(),
  getItem: (key) => values.get(key) ?? null,
  key: (index) => Array.from(values.keys())[index] ?? null,
  removeItem: (key) => {
    values.delete(key);
  },
  setItem: (key, value) => {
    values.set(key, String(value));
  },
};
vi.stubGlobal("localStorage", storage);
// App start-up and status polling can take over 1 s on slow CI runners.
configure({ asyncUtilTimeout: 3000 });
afterEach(cleanup);
