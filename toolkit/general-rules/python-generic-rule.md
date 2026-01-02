---
description: Python Development Standards for Robust and Secure Applications
globs:
  - "**/*.py"
  - "!**/migrations/**" # Exclude auto-generated migration files
  - "!**/__init__.py" # Often empty or just for imports, less rule-worthy
alwaysApply: true
---
# Python Core Development Standards

Rules promoting maintainable, secure, and efficient Python code, focusing on aspects beyond basic linting.

<rule>
name: python_typed_dict_or_dataclass_for_structured_data
description: Encourages using typing.TypedDict, @dataclasses.dataclass, or Pydantic models over raw dictionaries for structured data.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: suggest # Hard to universally reject dicts, but suggest for complex structures
    conditions:
      # Heuristic: functions returning dicts with multiple specific string keys
      - pattern: "def\\s+\\w+\\([^)]*\\)\\s*->\\s*dict:\\s*return\\s*{\\s*['\"]\\w+['\"]\\s*:\\s*[^,]+,\\s*['\"]\\w+['\"]\\s*:"
        message: "For functions returning structured data with multiple known keys, consider using `typing.TypedDict`, `@dataclasses.dataclass`, or a Pydantic model instead of a raw `dict` for better clarity and type safety."
    message: |
      When dealing with data structures that have a known set of keys and value types (e.g., API responses, internal state objects),
      using `typing.TypedDict`, `@dataclasses.dataclass`, or Pydantic `BaseModel` provides:
      - Improved readability and explicitness.
      - Type checking support with tools like MyPy.
      - Easier refactoring and maintenance.

      Example:
      ```python
      from typing import TypedDict
      from dataclasses import dataclass

      # Option 1: TypedDict
      class UserData(TypedDict):
          id: int
          name: str
          is_active: bool

      # Option 2: dataclass
      @dataclass
      class ProductInfo:
          sku: str
          price: float
          in_stock: int

      def get_user() -> UserData:
          return {"id": 1, "name": "Alice", "is_active": True}
      ```
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: python_api_key_security
description: Prevents hardcoding of API keys or sensitive credentials directly in the source code.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      # Looks for common API key patterns.
      - pattern: "(API_KEY|SECRET_KEY|ACCESS_TOKEN|_TOKEN|_SECRET)\\s*=\\s*['\"](sk-|rk_live|pk_test|ghp_|glpat-|[A-Za-z0-9\\-_\\.+]{20,})['\"]"
        message: "API keys or secrets must not be hardcoded. Use environment variables (e.g., `os.getenv('MY_API_KEY')`) or a secrets management system."
  - type: suggest
    message: |
      Never hardcode API keys, passwords, or other secrets directly in your Python files.
      Load them from environment variables (`os.getenv`) or a dedicated secrets management tool.
      This is critical for security, especially when code is shared or version controlled.

      Example:
      ```python
      import os
      # Good:
      api_key = os.getenv("OPENAI_API_KEY")
      if not api_key:
          raise ValueError("OPENAI_API_KEY environment variable not set.")

      # Bad:
      # OPENAI_API_KEY = "sk-thisIsAFakeKeyDoNotUseItReally"
      ```
examples:
  - input: |
      # my_service.py
      DATABASE_PASSWORD = "supersecretpassword123"
    output: "Rejected: API keys or secrets must not be hardcoded..."
  - input: |
      # my_service.py
      import os
      db_pass = os.getenv("DB_PASSWORD")
    output: "Accepted"
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: python_avoid_pickle_with_untrusted_data
description: Warns against using `pickle` to deserialize data from untrusted sources due to security risks.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content
    pattern: "pickle\\.(load|loads)"
actions:
  - type: reject
    conditions:
      # This is a strong warning. Actual untrusted source detection is hard.
      - pattern: "pickle\\.(load|loads)\\("
        message: "Deserializing data with `pickle.load/loads` from untrusted sources is insecure and can lead to arbitrary code execution. Use safer serialization formats like JSON or XML for external data, or ensure the pickle data source is fully trusted."
  - type: suggest
    message: |
      The `pickle` module is powerful but can execute arbitrary code when deserializing crafted payloads.
      If you are handling data from external users, files, or network requests, do NOT use `pickle`.
      Prefer safer alternatives:
      - `json` for simple data structures.
      - `xml.etree.ElementTree` for XML.
      - `marshmallow` or `pydantic` for complex object serialization/deserialization with validation.
      Only use `pickle` if you have absolute control and trust over the source of the pickled data.
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: python_avoid_mutable_default_arguments
description: Prevents the use of mutable default arguments in function definitions, which can lead to unexpected behavior.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      # Matches def func(arg=[] or arg={}):
      - pattern: "def\\s+\\w+\\s*\\([^)]*\\w+\\s*=\\s*(\\[\\]|\\{\\})[^)]*\\):"
        message: "Avoid using mutable default arguments (like `[]` or `{}`). Use `None` as the default and initialize the mutable type inside the function body if needed."
  - type: suggest
    message: |
      Default arguments are evaluated only once when the function is defined. If a mutable default (e.g., a list or dictionary) is modified,
      that modification will persist across subsequent calls to the function, leading to unexpected behavior.

      Example:
      ```python
      # Bad:
      # def append_to_list(element, my_list=[]):
      #     my_list.append(element)
      #     return my_list

      # Good:
      def append_to_list(element, my_list=None):
          if my_list is None:
              my_list = []
          my_list.append(element)
          return my_list
      ```
examples:
  - input: |
      # def bad_func(val, data=[]): pass
    output: "Rejected: Avoid using mutable default arguments..."
  - input: |
      # def good_func(val, data=None):
      #   if data is None: data = []
    output: "Accepted"
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: python_specific_exception_handling
description: Encourages catching specific exceptions rather than generic `Exception` or `BaseException`.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      - pattern: "except\\s+(Exception|BaseException)\\s*:" # Catches `except Exception:` or `except BaseException:`
        message: "Catch specific exceptions (e.g., `ValueError`, `IOError`) instead of generic `Exception` or `BaseException`. This prevents unintentionally catching system-exiting exceptions and allows for more targeted error handling."
  - type: suggest
    message: |
      Catching broad exceptions like `Exception` or `BaseException` can hide bugs and make debugging difficult.
      It can also catch exceptions you didn't intend to handle, like `KeyboardInterrupt` or `SystemExit`.
      Always try to catch the most specific exception type(s) relevant to the code in your `try` block.

      Example:
      ```python
      # Bad:
      # try:
      #     # ... some operation ...
      # except Exception as e:
      #     log.error(f"An unexpected error occurred: {e}")

      # Good:
      try:
          value = int(user_input)
      except ValueError:
          log.warning("Invalid input: Not a number.")
      except TypeError:
          log.warning("Invalid input: Expected a string or number.")
      ```
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: python_avoid_os_system_and_shell_true
description: Discourages use of `os.system()` and `subprocess` with `shell=True` due to security risks from command injection.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      - pattern: "os\\.system\\("
        message: "`os.system()` is a security risk as it invokes the system shell. Use the `subprocess` module with `shell=False` (the default) and pass arguments as a list."
      - pattern: "subprocess\\.(run|call|check_call|check_output|Popen)\\([^)]*shell\\s*=\\s*True"
        message: "Using `subprocess` functions with `shell=True` can lead to command injection if command parts come from external input. Prefer `shell=False` (default) and pass command arguments as a list."
  - type: suggest
    message: |
      Executing external commands via the system shell (`os.system` or `subprocess` with `shell=True`) can be dangerous if any part of the command string is derived from external or user input.
      It opens up vulnerabilities to command injection.
      The `subprocess` module is the recommended way to run external commands. Pass the command and its arguments as a list of strings, which avoids shell interpretation.

      Example:
      ```python
      import subprocess

      # Bad:
      # import os
      # user_filename = "some_file; rm -rf /" # Malicious input
      # os.system(f"cat {user_filename}")

      # Bad (subprocess with shell=True):
      # subprocess.run(f"cat {user_filename}", shell=True, check=True)

      # Good:
      user_filename = "some_file" # Assume validated/sanitized
      try:
          result = subprocess.run(["cat", user_filename], capture_output=True, text=True, check=True)
          print(result.stdout)
      except subprocess.CalledProcessError as e:
          print(f"Error executing command: {e}")
      ```
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: python_use_context_managers_for_resources
description: Recommends using context managers (`with` statement) for resources that need explicit cleanup (files, locks, database connections).
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      # Heuristic: Looks for open() not immediately followed by .read/write within a 'with' or followed by a .close()
      # This is tricky with regex; AST would be better.
      - pattern: "\\w+\\s*=\\s*open\\([^)]+\\)(?!.*close\\(\\))"
        message: "Files should be opened using a `with` statement (context manager) to ensure they are properly closed, even if errors occur. E.g., `with open(...) as f:`."
  - type: suggest
    message: |
      Resources like files, network connections, locks, and database connections often require explicit cleanup (e.g., closing a file).
      The `with` statement (context manager protocol) ensures that cleanup code (the `__exit__` method) is executed automatically,
      even if exceptions occur within the `with` block. This makes code cleaner and more robust.

      Example:
      ```python
      # Bad (might leave file open if error occurs before close):
      # f = open("my_file.txt", "r")
      # data = f.read()
      # # ... potential error here ...
      # f.close()

      # Good:
      try:
          with open("my_file.txt", "r") as f:
              data = f.read()
          # File is automatically closed here
      except FileNotFoundError:
          log.error("File not found.")
      ```
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: python_fstrings_for_formatting
description: Recommends using f-strings (formatted string literals) for string formatting in Python 3.6+.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: suggest # .format() and % are not "wrong", but f-strings are often preferred
    conditions:
      - pattern: "['\"]([^'{}]*\\{[^}]*\\}[^'{}]*)+\\.format\\(" # Catches "{}".format()
        message: "Consider using f-strings (e.g., `f\"Hello {name}\"`) for string formatting instead of `.format()` for improved readability and conciseness (Python 3.6+)."
      - pattern: "['\"]([^%]*%[sdifouxXeEgGcr%][^%]*)+['\"]\\s*%\\s*\\(" # Catches "%s" % (...)
        message: "Consider using f-strings (e.g., `f\"Value: {value}\"`) for string formatting instead of old-style %-formatting for better readability and type safety (Python 3.6+)."
    message: |
      F-strings (formatted string literals), introduced in Python 3.6, provide a concise and readable way to embed expressions inside string literals.
      They are generally preferred over older methods like `%` formatting or the `str.format()` method.

      Example:
      ```python
      name = "World"
      value = 42

      # Old style (%-formatting):
      # print("Hello %s, value is %d" % (name, value))

      # .format() method:
      # print("Hello {}, value is {}".format(name, value))

      # F-string (preferred):
      print(f"Hello {name}, value is {value}")
      ```
metadata:
  priority: low # More of a style preference
  version: 1.0
</rule>

<rule>
name: python_avoid_bare_except
description: Discourages using a bare `except:` clause as it catches all exceptions, including system-exiting ones.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      - pattern: "except\\s*:"
        message: "Avoid bare `except:` clauses. They catch all exceptions, including `SystemExit` and `KeyboardInterrupt`, making it hard to interrupt programs. Catch specific exceptions or at least `Exception` if you need to catch most program errors."
  - type: suggest
    message: |
      A bare `except:` clause will catch *all* exceptions, including `SystemExit`, `KeyboardInterrupt`, and `GeneratorExit`.
      This can make it difficult to terminate your program with Ctrl+C and can hide fundamental issues.
      If you need to catch a wide range of application errors, catch `Exception`. If you know the specific errors that might occur, catch those.

      Example:
      ```python
      # Bad:
      # try:
      #     # ...
      # except: # Catches EVERYTHING
      #     log.error("Something went wrong")

      # Better (catches most application errors):
      try:
          # ...
      except Exception as e:
          log.error(f"An application error occurred: {e}")

      # Best (catches specific, expected errors):
      try:
          user_id = int(input_data)
      except ValueError:
          log.warning("Invalid user ID format.")
      ```
metadata:
  priority: high
  version: 1.0
</rule>