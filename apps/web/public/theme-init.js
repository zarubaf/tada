// Applies the stored color theme before the first paint, so the page does not flash. The app
// script loads later and cannot do it in time. The key and the rule are those of src/theme/theme.ts.
(() => {
  try {
    const theme = window.localStorage.getItem("tada-theme");
    if (theme === "light" || theme === "dark") {
      document.documentElement.setAttribute("data-theme", theme);
    }
  } catch (_) {
    // No storage: the system decides.
  }
})();
