# Retro LaTeX-Style PDF Generation

Convert markdown documents to professional, retro LaTeX-style PDFs with academic formatting.

## Features

- **LaTeX/Academic styling** - Libre Baskerville font (similar to Computer Modern), tight paragraph spacing
- **Clickable Table of Contents** - Auto-generated TOC with anchor links to all sections
- **Geek Corner / Example Corner boxes** - Open-ended table style with horizontal rules (title separated from content by thin line)
- **Academic tables** - Horizontal rules only, italic headers, no vertical borders
- **Full references section** - Proper citations with clickable URLs
- **No headers/footers** - Clean pages without date/time stamps

## Usage

When the user asks to convert a markdown document to a retro/LaTeX-style PDF, follow this process:

### Step 1: Create the HTML Template

Create an HTML file with this exact structure and styling:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{{DOCUMENT_TITLE}}</title>
  <style>
    @import url('https://fonts.googleapis.com/css2?family=Libre+Baskerville:ital,wght@0,400;0,700;1,400&display=swap');

    body {
      font-family: 'Libre Baskerville', 'Computer Modern', Georgia, serif;
      font-size: 10.5pt;
      line-height: 1.35;
      max-width: 680px;
      margin: 40px auto;
      padding: 0 20px;
      color: #000;
      background: #fff;
    }

    h1 {
      font-size: 16pt;
      font-weight: bold;
      text-align: center;
      margin-bottom: 6px;
      line-height: 1.2;
    }

    .subtitle {
      text-align: center;
      font-style: italic;
      font-size: 9.5pt;
      margin-bottom: 20px;
      color: #000;
    }

    h2 {
      font-size: 12pt;
      font-weight: bold;
      margin-top: 18px;
      margin-bottom: 8px;
    }

    h3 {
      font-size: 10.5pt;
      font-weight: bold;
      margin-top: 14px;
      margin-bottom: 6px;
    }

    h4 {
      font-size: 10.5pt;
      font-weight: bold;
      font-style: italic;
      margin-top: 12px;
      margin-bottom: 5px;
    }

    p {
      margin-bottom: 6px;
      text-align: justify;
      text-justify: inter-word;
    }

    ul, ol {
      margin: 4px 0 6px 20px;
      padding: 0;
    }

    li {
      margin-bottom: 2px;
    }

    /* Table of Contents */
    .toc {
      margin: 20px 0 30px 0;
    }

    .toc-title {
      font-weight: bold;
      font-size: 11pt;
      margin-bottom: 10px;
    }

    .toc ul {
      list-style: none;
      margin: 0;
      padding: 0;
    }

    .toc li {
      margin-bottom: 3px;
    }

    .toc a {
      color: #000;
      text-decoration: none;
    }

    .toc a:hover {
      text-decoration: underline;
    }

    .toc .toc-h2 {
      margin-left: 0;
    }

    .toc .toc-h3 {
      margin-left: 15px;
      font-size: 9.5pt;
    }

    /* Geek Corner / Example Corner - open table style like LaTeX */
    .geek-corner {
      margin: 14px 0;
      padding: 0;
    }

    .geek-corner-top {
      border-top: 1.5px solid #000;
      padding-top: 8px;
      padding-bottom: 6px;
      border-bottom: 1px solid #000;
    }

    .geek-corner-bottom {
      border-bottom: 1.5px solid #000;
      padding-top: 6px;
      padding-bottom: 8px;
    }

    .geek-corner-title {
      font-weight: bold;
    }

    .geek-corner-content {
      font-style: italic;
    }

    blockquote {
      margin: 10px 0 10px 0;
      padding: 0 0 0 20px;
      font-style: italic;
    }

    code {
      font-family: "Courier New", Courier, monospace;
      font-size: 9.5pt;
    }

    /* Tables - open style with horizontal rules only */
    table {
      width: 100%;
      border-collapse: collapse;
      margin: 12px 0;
      font-size: 10pt;
    }

    th {
      border-top: 1.5px solid #000;
      border-bottom: 1px solid #000;
      padding: 6px 8px;
      text-align: left;
      font-weight: normal;
      font-style: italic;
    }

    td {
      padding: 5px 8px;
      border-bottom: 1px solid #000;
      vertical-align: top;
    }

    tr:last-child td {
      border-bottom: 1.5px solid #000;
    }

    hr {
      border: none;
      border-top: 1px solid #000;
      margin: 20px 0;
    }

    strong {
      font-weight: bold;
    }

    em {
      font-style: italic;
    }

    .footnote {
      font-size: 8.5pt;
      margin-top: 30px;
      padding-top: 10px;
      border-top: 1px solid #000;
    }

    .footnote a {
      color: #000;
      word-break: break-all;
    }

    sup {
      font-size: 7pt;
    }

    sup a {
      color: #000;
      text-decoration: none;
    }

    .key-takeaway {
      margin: 10px 0;
    }

    .key-takeaway strong {
      font-weight: bold;
    }

    @media print {
      body {
        margin: 0;
        padding: 15px;
        font-size: 10pt;
      }
      @page {
        margin: 1.8cm;
        size: A4;
      }
      a {
        color: #000;
      }
    }
  </style>
</head>
<body>
  <!-- CONTENT GOES HERE -->
</body>
</html>
```

### Step 2: Structure the Content

#### Title and Subtitle
```html
<h1>Document Title</h1>
<p class="subtitle">Subtitle or tagline in italics</p>
<hr>
```

#### Table of Contents
```html
<div class="toc">
  <div class="toc-title">Contents</div>
  <ul>
    <li class="toc-h2"><a href="#sec1">1. Section Title</a></li>
    <li class="toc-h3"><a href="#sec1-1">1.1 Subsection Title</a></li>
    <!-- ... more entries ... -->
    <li class="toc-h2"><a href="#references">References</a></li>
  </ul>
</div>
<hr>
```

#### Section Headings (with anchor IDs)
```html
<h2 id="sec1">1. Section Title</h2>
<h3 id="sec1-1">1.1 Subsection Title</h3>
<h4>Step 1: Sub-subsection (italic)</h4>
```

#### Geek Corner / Example Corner Box
```html
<div class="geek-corner">
  <div class="geek-corner-top">
    <span class="geek-corner-title">Geek Corner: Title Here</span>
  </div>
  <div class="geek-corner-bottom">
    <span class="geek-corner-content">"The witty or insightful quote goes here in italics."</span>
  </div>
</div>
```

#### Tables (Academic Style)
```html
<table>
  <tr>
    <th>Column 1</th>
    <th>Column 2</th>
    <th>Column 3</th>
  </tr>
  <tr>
    <td>Data</td>
    <td>Data</td>
    <td>Data</td>
  </tr>
</table>
```

#### Footnote References (in text)
```html
<sup><a href="#ref1">[1]</a></sup>
```

#### References Section
```html
<hr>
<h2 id="references">References</h2>
<div class="footnote" style="border-top: none; margin-top: 10px; padding-top: 0;">
  <p id="ref1"><strong>[1]</strong> Author, Name. "Article Title." Publication (Year). <a href="https://url.com" target="_blank">https://url.com</a> — Brief description.</p>
  <p id="ref2"><strong>[2]</strong> ...</p>
</div>
```

#### Key Takeaway
```html
<p class="key-takeaway"><strong>Key Takeaway:</strong> Important summary text here.</p>
```

### Step 3: Generate the PDF

Use Chrome headless to generate the PDF without headers/footers:

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless \
  --disable-gpu \
  --print-to-pdf="/path/to/output.pdf" \
  --print-to-pdf-no-header \
  --no-pdf-header-footer \
  "file:///path/to/input.html"
```

On Linux:
```bash
google-chrome --headless --disable-gpu --print-to-pdf="/path/to/output.pdf" --print-to-pdf-no-header --no-pdf-header-footer "file:///path/to/input.html"
```

## Style Guidelines

1. **Paragraphs**: Keep text dense and justified. Collapse bullet points into flowing prose where appropriate.

2. **Geek Corner**: Use for witty asides, technical insights, or memorable quotes. Always include:
   - A catchy title (e.g., "Geek Corner: Amdahl's Law, App Edition")
   - Italic content in quotes

3. **Tables**: Use sparingly for data comparisons. Always use the open style (horizontal rules only).

4. **References**: Always include full citations with:
   - Author name
   - Article/document title in quotes
   - Publication/source and year
   - Full clickable URL
   - Brief description of what the reference covers

5. **Spacing**: Keep tight - 6px paragraph margins, 1.35 line height.

## Example Output

See `/Users/stevengonsalvez/d/btg/multibrand/write-up-simple.html` for a complete example of this formatting applied to a technical write-up.
