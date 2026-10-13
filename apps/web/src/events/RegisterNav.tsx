import { t } from "../i18n";
import { NavLink, SubNav } from "../ui/NavLink";
import { useEventContext } from "./eventContext";

/**
 * The registers of an event, in the order of the second row of the event navigation
 * (doc/design/layout-and-responsiveness.md, „Navigation“). A new register adds one line here.
 */
const REGISTERS = [
  { segment: "commitments", label: "register-nav-commitments" },
  { segment: "milestones", label: "register-nav-milestones" },
] as const;

/** The paths of the registers of the event at `base`. The first one opens „Register“. */
export function registerPaths(base: string): readonly [string, ...string[]] {
  const [first, ...others] = REGISTERS;
  const path = ({ segment }: { segment: string }) => `${base}/${segment}`;
  return [path(first), ...others.map(path)];
}

/** The second row of the event navigation below „Register“: one tab for each register. */
export function RegisterNav() {
  const { event } = useEventContext();
  const base = `/events/${encodeURIComponent(event.id)}`;
  return (
    <SubNav label={t("register-nav")} level="secondary">
      {REGISTERS.map(({ segment, label }) => (
        <NavLink key={segment} to={`${base}/${segment}`}>
          {t(label)}
        </NavLink>
      ))}
    </SubNav>
  );
}
