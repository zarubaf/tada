// Components use only design tokens (ADR 0019). A raw value fails the check.
// Only tokens.css defines raw values.
export default {
  extends: ["stylelint-config-standard"],
  plugins: ["stylelint-declaration-strict-value"],
  rules: {
    "selector-class-pattern": null,
    // CSS Modules: `:global(html)` reaches the root element from a component module.
    "selector-pseudo-class-no-unknown": [true, { ignorePseudoClasses: ["global"] }],
    // Plain numbers, as in doc/design/tokens.md.
    "hue-degree-notation": "number",
    "alpha-value-notation": "number",
    "scale-unlimited/declaration-strict-value": [
      [
        "/color$/",
        "fill",
        "stroke",
        "/^(margin|padding|gap|row-gap|column-gap|inset)/",
        "/radius$/",
        "font-size",
        "box-shadow",
        "transition-duration",
        "z-index",
      ],
      {
        ignoreValues: [
          "0",
          "auto",
          "currentcolor",
          "inherit",
          "initial",
          "none",
          "transparent",
          "unset",
          // The system colors of forced colors (ADR 0022, doc/design/accessibility.md).
          "/^(Canvas|CanvasText|GrayText|Highlight|HighlightText)$/",
        ],
        ignoreFunctions: false,
      },
    ],
  },
  overrides: [
    {
      files: ["src/styles/tokens.css"],
      rules: { "scale-unlimited/declaration-strict-value": null },
    },
  ],
};
