import { Component, ReactNode, ErrorInfo } from "react";
import { Button } from "./ui";

interface Props {
  children: ReactNode;
  name?: string;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(`[ErrorBoundary${this.props.name ? `:${this.props.name}` : ""}]`, error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="app-shell">
          <div className="centered-view">
            <div className="error-boundary">
              <div className="error-boundary__icon">⚠️</div>
              <h2 className="centered-view__title centered-view__title--spaced">
                {this.props.name || "View"} Crashed
              </h2>
              <p className="centered-view__desc">
                {this.state.error?.message || "An unexpected error occurred."}
              </p>
              <Button onClick={() => { this.setState({ hasError: false, error: null }); window.location.reload(); }}>
                Reload
              </Button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
