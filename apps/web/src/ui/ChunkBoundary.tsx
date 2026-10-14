import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  fallback: ReactNode;
  /** The failure ends when this value changes, for example with the path of the address. */
  resetKey?: string;
}

interface State {
  failed: boolean;
  resetKey: string | undefined;
}

/** Catches the failure of a chunk, for example after a new release removed it. */
export class ChunkBoundary extends Component<Props, State> {
  override state: State = { failed: false, resetKey: this.props.resetKey };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  static getDerivedStateFromProps(props: Props, state: State): Partial<State> | null {
    return props.resetKey === state.resetKey ? null : { failed: false, resetKey: props.resetKey };
  }

  override render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}
