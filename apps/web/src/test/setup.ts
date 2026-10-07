import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(cleanup);

// The first render of a file is slow while the machine runs the other checks.
configure({ asyncUtilTimeout: 5000 });
