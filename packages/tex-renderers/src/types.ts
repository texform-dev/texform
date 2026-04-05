export interface CompileOptions {
  packages: string[];
  mode: "math" | "text";
  display?: boolean;
}

export interface CompileResult {
  success: boolean;
  error?: string;
  output?: string;
}

export interface TexCompiler {
  name: "mathjax" | "katex" | "xetex";
  compile(tex: string, options: CompileOptions): Promise<CompileResult>;
  dispose?(): Promise<void>;
}
