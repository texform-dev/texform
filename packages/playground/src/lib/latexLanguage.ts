import type * as Monaco from 'monaco-editor'

export const LATEX_LANGUAGE_ID = 'latex'

export function registerLatexLanguage(monaco: typeof Monaco): void {
  monaco.languages.register({ id: LATEX_LANGUAGE_ID })

  // Monarch tokenizer for LaTeX math formulas
  monaco.languages.setMonarchTokensProvider(LATEX_LANGUAGE_ID, {
    tokenizer: {
      root: [
        // Comments
        [/%.*$/, 'comment'],
        // Commands: \commandname or \commandname*
        [/\\[a-zA-Z@]+\*?/, 'keyword'],
        // Special single-char escapes: \\, \{, \}, \$, etc.
        [/\\[^a-zA-Z\s]/, 'keyword'],
        // Curly braces
        [/[{}]/, 'delimiter.curly'],
        // Square brackets
        [/[\[\]]/, 'delimiter.square'],
        // Parentheses
        [/[()]/, 'delimiter.paren'],
        // Math structural operators: subscript, superscript, alignment
        [/[_^&]/, 'operator'],
        // Numbers
        [/\d+(?:\.\d+)?/, 'number'],
      ],
    },
  })

  // Light theme matching GitHub Primer light tokens
  monaco.editor.defineTheme('texform-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6e7781', fontStyle: 'italic' },
      { token: 'keyword', foreground: '0550ae' },
      { token: 'operator', foreground: '953800' },
      { token: 'number', foreground: '0969da' },
      { token: 'delimiter.curly', foreground: '1f2328' },
      { token: 'delimiter.square', foreground: '6e7781' },
      { token: 'delimiter.paren', foreground: '6e7781' },
    ],
    colors: {
      'editor.background': '#ffffff',
      'editor.foreground': '#1f2328',
      'editorLineNumber.foreground': '#6e7781',
      'editorLineNumber.activeForeground': '#1f2328',
      'editorCursor.foreground': '#0969da',
      'editor.selectionBackground': '#0969da33',
      'editor.lineHighlightBackground': '#f6f8fa',
      'editorGutter.background': '#ffffff',
      'editorError.foreground': '#d1242f',
      'editorError.border': '#00000000',
      'editorWarning.foreground': '#9a6700',
    },
  })

  // Dark theme matching GitHub Primer dark tokens
  monaco.editor.defineTheme('texform-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '8b949e', fontStyle: 'italic' },
      { token: 'keyword', foreground: '79c0ff' },
      { token: 'operator', foreground: 'ffa657' },
      { token: 'number', foreground: '79c0ff' },
      { token: 'delimiter.curly', foreground: 'e6edf3' },
      { token: 'delimiter.square', foreground: '8b949e' },
      { token: 'delimiter.paren', foreground: '8b949e' },
    ],
    colors: {
      'editor.background': '#0d1117',
      'editor.foreground': '#e6edf3',
      'editorLineNumber.foreground': '#6e7681',
      'editorLineNumber.activeForeground': '#e6edf3',
      'editorCursor.foreground': '#58a6ff',
      'editor.selectionBackground': '#58a6ff33',
      'editor.lineHighlightBackground': '#161b22',
      'editorGutter.background': '#0d1117',
      'editorError.foreground': '#f85149',
      'editorError.border': '#00000000',
      'editorWarning.foreground': '#d29922',
    },
  })
}
