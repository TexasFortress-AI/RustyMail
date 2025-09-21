# RustyMail Product Requirements Document (PRD)

## Executive Summary

RustyMail is a high-performance, type-safe IMAP API server written in Rust that provides multiple interfaces (REST, MCP stdio, MCP SSE) for accessing email functionality. The project is in active development with substantial progress made on core functionality, but requires focused effort to complete remaining features and resolve architectural issues.

## Project Vision

RustyMail aims to be a production-ready, robust IMAP middleware solution that:
- Provides a unified API for email operations across multiple transport protocols
- Offers real-time monitoring and management through a web dashboard
- Includes AI-powered natural language interfaces for email interactions
- Maintains high performance and reliability standards
- Supports extensibility through the Model Context Protocol (MCP)

## Current Status

### Completed Features
✅ **Core Infrastructure**
- Basic IMAP client implementation with async operations
- Three API interfaces: REST, MCP stdio, MCP SSE
- Session management and connection pooling
- Authentication and TLS support
- Basic error handling framework

✅ **Dashboard Backend (90% Complete)**
- REST API endpoints for stats, clients, config, and chatbot
- SSE event streaming for real-time updates
- Metrics collection service
- Client management service
- Configuration service
- AI service with OpenAI/OpenRouter integration (ports/adapter pattern)
- Integration with main application

✅ **Dashboard Frontend (UI Plan Complete)**
- Comprehensive UI specification with Steve Jobs-inspired UX
- React + Vite + TypeScript architecture defined
- Component specifications complete
- Mock data implementation planned
- SSE integration design complete

### In Progress / Partially Complete

🔄 **MCP Library Migration**
- Need to migrate from custom MCP implementation to official rust-sdk
- Current implementation works but lacks standards compliance
- Migration will improve maintainability and interoperability

🔄 **Testing Infrastructure**
- Basic unit tests exist
- Integration tests for dashboard SSE implemented
- Need comprehensive E2E test coverage
- Mock IMAP adapter partially implemented

🔄 **AI Chatbot Integration**
- Backend AI service implemented with provider pattern
- RIG integration planned for LLM capabilities
- Natural language processing pipeline defined
- Frontend chat interface specified

## Critical Issues to Address

### 1. IMAP Type System Inconsistency (HIGHEST PRIORITY)
**Problem:** The codebase uses two competing IMAP libraries (`async-imap` and `imap-types`) simultaneously, causing:
- Duplicate type definitions with same names
- Incomplete/incorrect type conversions
- Type ambiguity throughout codebase
- Conversion overhead and bugs

**Solution Required:**
- Choose one primary IMAP library
- Create clear abstraction layer
- Define domain-specific types independent of library

### 2. Session Management Complexity
**Problem:** Overly complex session architecture with multiple patterns:
- Multiple wrapper types (AsyncImapSessionWrapper, ImapClient<T>)
- Confusing factory patterns with similar names
- Inconsistent mutex usage patterns
- Potential for concurrency bugs

**Solution Required:**
- Simplify session creation pattern
- Standardize session management approach
- Make factory pattern explicit and less generic

### 3. Error Handling Inconsistencies
**Problem:** Lack of standardized error handling:
- Incomplete error mapping between types
- Multiple conversion mechanisms
- Inconsistent error context
- Unclear async error handling

**Solution Required:**
- Create comprehensive error type mapping
- Standardize conversion approach
- Improve error context and messages

## Development Roadmap

### Phase 1: Critical Bug Fixes (Week 1-2)
**Priority: CRITICAL**
1. **IMAP Type System Consolidation**
   - Choose between `async-imap` or `imap-types`
   - Refactor all IMAP operations to use single library
   - Create domain abstraction layer
   - Update all type conversions

2. **Session Management Simplification**
   - Consolidate session wrapper types
   - Simplify factory pattern
   - Standardize mutex usage
   - Add proper documentation

3. **Error Handling Standardization**
   - Create unified error type hierarchy
   - Implement comprehensive error mapping
   - Add context to all errors
   - Document error codes

### Phase 2: MCP SDK Migration (Week 2-3)
**Priority: HIGH**
1. Add rust-sdk dependency
2. Migrate transport implementations
3. Refactor service definitions using SDK tooling
4. Update type definitions
5. Create compatibility layer if needed
6. Comprehensive testing of migrated functionality

### Phase 3: Complete Dashboard Implementation (Week 3-4)
**Priority: HIGH**
1. **Frontend Development**
   - Set up React + Vite + TypeScript project
   - Implement component library with shadcn/ui
   - Create stats, clients, chatbot, and config components
   - Implement SSE real-time updates
   - Add IMAP adapter selector with persistence

2. **Backend Refinements**
   - Fix ClientManager cleanup bug
   - Ensure proper test process cleanup
   - Connect real IMAP metrics (currently using proxy)
   - Optimize performance

### Phase 4: AI Chatbot Implementation (Week 4-5)
**Priority: MEDIUM**
1. **RIG Integration**
   - Set up RIG for LLM inference
   - Configure OpenRouter/Deepseek provider
   - Implement environment-based configuration

2. **Core Chatbot Logic**
   - Natural language processing pipeline
   - MCP tool definitions for LLM
   - Conversation context management
   - Response formatting

3. **Adapter Implementation**
   - StdioAIChatbotAdapter for terminal
   - SseAIChatbotAdapter for dashboard
   - Security and permissions system

### Phase 5: Comprehensive Testing (Week 5-6)
**Priority: HIGH**
1. **Test Infrastructure**
   - Implement MockImapAdapter fully
   - Add GoDaddyImapAdapter for live testing
   - Set up Cucumber for BDD testing

2. **E2E Test Suites**
   - MCP stdio API tests
   - MCP SSE API tests
   - AI chatbot interaction tests
   - Dashboard UI tests with browser automation

3. **Performance Testing**
   - Load testing for concurrent connections
   - Memory leak detection
   - Response time optimization

### Phase 6: Documentation & Polish (Week 6-7)
**Priority: MEDIUM**
1. **Documentation**
   - Complete API documentation
   - Update README with latest features
   - Create deployment guide
   - Add troubleshooting guide

2. **Code Quality**
   - Run comprehensive linting
   - Address all clippy warnings
   - Add missing unit tests
   - Code review and refactoring

3. **UI/UX Polish**
   - Implement animations and transitions
   - Add accessibility features
   - Dark mode support
   - Mobile responsiveness

## Success Metrics

1. **Functionality**
   - All IMAP operations working correctly
   - Dashboard fully operational with real-time updates
   - AI chatbot successfully processing natural language queries
   - All three API interfaces (REST, stdio, SSE) stable

2. **Performance**
   - Handle 100+ concurrent connections
   - Response time < 200ms for standard operations
   - Memory usage stable under load
   - No memory leaks

3. **Quality**
   - 80%+ test coverage
   - Zero critical bugs
   - All error scenarios handled gracefully
   - Clean clippy output

4. **Usability**
   - Dashboard intuitive and responsive
   - Clear documentation for all features
   - Easy deployment process
   - Helpful error messages

## Resource Requirements

1. **Development Team**
   - 1-2 Rust developers for backend
   - 1 React developer for frontend
   - 1 QA engineer for testing

2. **Infrastructure**
   - Development IMAP test accounts
   - CI/CD pipeline
   - Testing environments

3. **Third-Party Services**
   - OpenAI/OpenRouter API keys for AI features
   - IMAP test accounts (Gmail, Outlook, GoDaddy)

## Risk Analysis

1. **Technical Risks**
   - IMAP library consolidation may reveal deep dependencies
   - Performance issues under high load
   - Browser compatibility for dashboard

2. **Schedule Risks**
   - Type system refactoring may take longer than estimated
   - Integration testing with live IMAP servers complex
   - AI chatbot accuracy may require tuning

3. **Mitigation Strategies**
   - Create feature flags for gradual rollout
   - Maintain backward compatibility during migration
   - Extensive testing before production deployment

## Future Enhancements (Post-MVP)

1. **Advanced Features**
   - OAuth support for Gmail/Outlook
   - Email composition and sending
   - Attachment handling
   - Advanced search capabilities

2. **Dashboard Enhancements**
   - Historical metrics storage
   - Advanced analytics
   - Alerting system
   - User management

3. **AI Capabilities**
   - Multi-model support
   - Custom training for email patterns
   - Automated email categorization
   - Smart reply suggestions

## Conclusion

RustyMail is a promising project with solid foundation but requires focused effort to address critical architectural issues and complete remaining features. The highest priority is resolving the IMAP type system inconsistency, followed by completing the dashboard implementation and adding comprehensive testing. With dedicated development effort following this roadmap, RustyMail can become a production-ready solution within 6-7 weeks.

The project's modular architecture and clear separation of concerns provide a good foundation for future enhancements. Once the critical issues are resolved and core features completed, RustyMail will offer a unique combination of performance, reliability, and user-friendly interfaces for email operations.