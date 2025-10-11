# Claude Code Instructions

## Task Master AI Instructions
**Import Task Master's development workflow commands and guidelines, treat as if import is in the main CLAUDE.md file.**
@./.taskmaster/CLAUDE.md

Make every task and code change you do as simple as possible. We want to avoid making any massive or complex changes. Every change should impact as little code as possible. Everything is about simplicity.

DO NOT BE LAZY. NEVER BE LAZY. IF THERE IS A BUG FIND THE ROOT CAUSE AND FIX IT. NO TEMPORARY FIXES. YOU ARE A SENIOR DEVELOPER. NEVER BE LAZY

## Working with This Repository

Don't ever just code "off the top of your head" -- use the mcp servers to get the context and code visibility.
Don't assume something is working if you aren't directly testing it yourself.
Don't assume something is building if you haven't directly built it yourself!
Don't assume the UI is displaying correctly if you haven't actually used a browser to verify you can see it yourself that it's working!

IMPORTANT:
- Remember to always use Context7 and rust-docs for looking up Documentation of any classes you're using.
- Remember to always use tree-sitter for code visibility of the project.
- Remember to always use mycelian-memory to keep track of your understanding of the project goals and decisions, key insights, lessons learned. Also, search your memory for relevant information BEFORE making an important decision.
- Remember to use your puppeteer tool for testing the frontend in the browser.

WARNING: Don't use over-generalized searches in the code base, because the results are too large for the context window. Be specific and targeted.

REMEMBER: The PURPOSE of your memory is to keep track of key insights, and decision points, so you can use them to make informed decisions. When you have to make a decision, you should first check for relevant memories. And once the decision is made, you should always remember WHY you made that decision, and WHAT the decision was, and how it relates to the overall goal, and the KEY INSIGHTS derived along the way. Don't store tasks in your memory, that's what the taskmaster is for.

REMEMBER: When I ask you about something we've already discussed, you should check your memories, in addition to searching the code, to find the answer.

Remember: When you need to make a decision, consult your memory for relevant information, since it will often inform your decision.

IMPORTANT: Remember to use taskmaster for tracking all tasks and progress. (use "taskmaster next" to see next task).
IMPORTANT: Make sure the project always builds and passes all tests before continuing to the next task.
IMPORTANT: If you make any schemas, (SQL or JSON or anything else) save that sort of thing in the project so we can commit it to git.
IMPORTANT: Once a task is complete and builds, and passes all the tests, make sure to update the taskmaster with the new status.
IMPORTANT: Then, always commit to git before continuing to the next task.

## Do not ever use Common port numbers

Common port numbers like 8000, 8080, 5000, 3000 (etc.) are going to be in use by other processes. There's a docker running on this computer and all kinds of other apps. Do not EVER use common port numbers! Pick a port number that is not in use and is not a common port number. Furthermore, explicitly ensure that you aren't using any port numbers that conflict with ports in use by other dockers that are running on this computer. When you pick a port number for some purpose, check the .env.example file first, and always create an environment variable in the .env.example file for that port number.

If you just got the backend server re-built, make sure to stop the old server and start the new one. Clean and fresh.

Same with the front-end. Rebuild after changes, and restart the front end after any rebuilds.

## Docker desktop is installed

Wherever appropriate, use services in a docker rather than installing directly to the machine. 

## Don't hard code any values

Don't hard code any values. Use environment variables or config files.

# Rules for writing code / making specific edits

- Only make one logical change to the code at a time. Be methodical.
- Keep these changes small. (Example: A 10-line change, rather than re-writing an entire file).
- BE PRECISE. Never make assumptions about a class. If you can't see the exact definition or documentation of a class you're using, then ASK. When in doubt, stop and ask the user before making code changes!
- NO HARDCODING! Instead, use constants, environment variables, config files, etc. Do NOT hardcode values inside the code.
- USE WHAT WORKS. If existing code already compiles and works, then new additions should be modeled on it. For example, if we already have four working tools, and then we add a fifth tool, then it should work within the same proven framework of the other existing tools. If all the other tools that are KNOWN to WORK, log a certain way, then the new tool should ALSO log the same way. Etc.
- NEVER use placeholders in the code.
    - Negative examples (what to avoid):
        - "// Remainder of the code stays the same"
        - "# The rest of this function remains the same"
        - "    # Keep the original version of the code below this point."
        - "//... (rest of the original code remains the same)"
    - The reason we can't ever use placeholders: Placeholders will cause the editor to accidentally overwrite pre-existing code with a placeholder! This is very bad, and so we NEVER want this to happen! We never want to accidentally erase code that was already previously completed. (Right?) Therefore: All code changes should be small enough, and specific enough, that placeholders should NEVER be warranted. Do NOT use placeholders EVER when referencing pre-existing code that was already written.


# Rules for architecting / designing code

1. ASK. When uncertain what is the right move, just ask the user first for advice or permission, before moving forward with more changes. 
2. DO IT RIGHT. Always choose the simplest, cleanest, and most correct and elegant way to do something. Never add unnecessary features outside of specification. Avoid unnecsessary complexity.
3. INCLUDE/IMPORT WHAT YOU USE.If you're going to use a class, make sure you include/import it appropriately so we don't get a build or runtime error when we test it.
4. DON'T OVER-ENGINEER. No overkill, no crazy unnecessary features before core functionality is complete first. Always strive for the minimal working example, the minimum viable product. MVP!
5. TEST ALL FUNCTIONALITY. When adding new functionality, make sure you also add a new test for that functionality.
6. TESTS MUST PASS BEFORE MAKING A GIT COMMIT. Before making a git commit, make sure the code passes the unit tests before making any new changes. If the unit tests are failing, then no changes should be made other than fixing the bugs revealed by executing the unit tests. If we have to, we'll roll back the code to a previous commit before we will ever commit broken code.
7. DON'T CHEAT. Unit tests should always prove whether or not a piece of functionality works AS INTENDED. Meaning you should NEVER falsely change a unit test so that it appears to pass when the actual functionality being tested is still broken. This is cheating, and it will cause you many more problems in the future. The only acceptable changes to a unit test are fixes intended to make sure it works correctly in proving whether the functionality being tested really works or not. Other than that, if the tests are failing, then the fix for that should always be in the actual functionality being tested, and not in the test itself.
8. HIGHEST POSSIBLE LEVEL OF ABSTRACTION. Always use the highest-level interface, with the highest-level abstraction, that's appropriate/possible in every situation. If you find yourself using a lower-level interface than really necessary, then you're probably doing something wrong. So, explain clearly when choosing which interface to use, and articulate your reasoning into words so that I understand your intentions. Any reasonable developer should agree with your choices.
9. CLARIFY YOUR INTENTIONS. When you make a change, always explain to me in plain english what you are doing, why you're doing it, and what the effect of the change is going to be. How does the change fit into your overall plan? You must be able to articulate your specific intentions INTO WORDS. What's the big picture?
10. EXPLAIN YOURSELF. Before you change any code regarding the use of any specific class, first explain to me in plain english showing me the exact method / parameter profile / function definition / etc. that you intend to use. This information can ONLY come from the actual class definition/documentation, which you must reference when you give me your explanation. If you can't do that, then you have no business making those changes to the code in the first place! That's exactly the situation where you should ASK THE USER to provide the exact definition.
11. FIX ALL INSTANCES of a bug. Once you have identified a certain problem in the code, make sure you fix all the places where that problem occurs, and not just the first one you found. We don't want to have to go back over and over again fixing the same bug multiple times. Once it's been identified, fix it everywhere that it occurs so we can move on with our lives.
12. MINIMIZE SURPRISES. Be up front about what you are EXPECTING to happen as a result of your changes. Meaning: When you make a change, first tell the user specifically what effect you expect that change to have when we build and test the code. If it turns out that that intended effect is not what ACTUALLY happens, then we need to re-examine our thinking that caused us to make that change in the first place!
13. ARTICULATE. Use specific, articulable facts. No vague languaging. Be SPECIFC about exactly WHAT you perceive, WHAT you are changing, and WHY you are changing it, and HOW that matters in the scheme of things, or HOW it relates to what's going on. This is just like the legal concept of 'probable cause' or 'reasonable suspicion': you MUST be able to articulate the specifics INTO WORDS. Don't just say, (for example) "the problem is in how we're handling the parameters" because that conveys ZERO information about what the problem actually is. BE SPECIFIC!
    - Negative examples / what to avoid (parenthetical describes why it's negative):
        - "the problem is in how we coded it" (Fails to specify what the problem actually is)
        - "the test is failing because we're not properly handling the parameters" (Fails to specify what precisely is handled wrong with the parameters)
        - "we're not properly handling the response" (Fails to specify what exactly is improperly handled)
        - "The main problem seems to be in how we're handling the RPC response" (Makes a claim but doesn't explain why)
        - "Now we're seeing the actual error in the edit request" (Doesn't explain what the actual error is)
        - "The Router API is different than what we assumed." (Doesn't explain HOW it's different)


<file_length_and_structure›
- Never allow a file to exceed 500 lines.
- If a file approaches 400 lines, break it up immediately.
- Treat 1000 lines as unacceptable, even temporarily.
- Use folders and naming conventions to keep small files logically grouped.
‹/file_length_and_structure›

‹oop-first›
- Every functionality should be in a dedicated class, struct, or protocol, even if it's small.
- Favor composition over inheritance, but always use object-oriented thinking.
- Code must be built for reuse, not just to "make it work."
‹/oop_first›

‹single_responsibility-principle>
- Every file, class, and function should do one thing only.
- If it has multiple responsibilities, split it immediately.
- Each view, manager, or utility should be laser-focused on one concern.
‹/single_responsibility-principle>

‹modular-design›
- Code should connect like Lego - interchangeable, testable, and isolated.
- Ask: "Can I reuse this class in a different screen or project?" If not, refactor it.
- Reduce tight coupling between components. Favor dependency injection or protocols.
‹/modular_design>

‹manager_and_coordinator_patterns›
- Use ViewModel, Manager, and Coordinator naming conventions for logic separation:
    - UI logic → ViewModel
    - Business logic → Manager
    - Navigation/state flow → Coordinator
- Never mix views and business logic directly.
‹/manager_and_coordinator-patterns>

‹function_and_class_size›
- Keep functions under 30-40 lines.
- If a class is over 200 lines, assess splitting into smaller helper classes.
‹/function_and_class-size›

‹naming_and_readability›
- All class, method, and variable names must be descriptive and intention-revealing.
- Avoid vague names like data, info, helper, or temp.
‹/naming_and_readability›

‹scalability-mindset›
- Always code as if someone else will scale this.
- Include extension points (e.g., protocol conformance, dependency injection) from day one.
‹/scalability-mindset›

‹avoid_god_classes>
- Never let one file or class hold everything (e.g., massive ViewController, ViewModel, or Service).
- Split into Ul, State, Handlers, Networking, etc.
‹/avoid_god_classes›
