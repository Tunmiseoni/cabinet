import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("The Cabinet UI error:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex min-h-screen flex-col items-center justify-center gap-4 bg-neutral-950 p-8 text-center text-neutral-100">
          <h1 className="text-lg font-semibold">Something went wrong</h1>
          <pre className="max-w-2xl overflow-auto rounded-md bg-neutral-900 p-4 text-left text-xs text-amber-400">
            {this.state.error.message}
          </pre>
          <button
            type="button"
            className="rounded-md border border-neutral-700 px-3 py-1.5 text-sm"
            onClick={() => this.setState({ error: null })}
          >
            Dismiss
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
