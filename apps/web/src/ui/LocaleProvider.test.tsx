import { render, screen } from "@testing-library/react";
import { useLocale } from "react-aria-components";
import { expect, it } from "vitest";
import { LocaleProvider } from "./LocaleProvider";

function Locale() {
  return <p>{useLocale().locale}</p>;
}

it("fixes the locale of React Aria to German, whatever the browser says", () => {
  render(
    <LocaleProvider>
      <Locale />
    </LocaleProvider>,
  );

  expect(screen.getByText("de-CH")).toBeInTheDocument();
});
