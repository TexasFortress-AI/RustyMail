-- Who is behind a proposal's message: its sender, or for mail a platform
-- delivers on someone's behalf (document sharing, e-signature) the person in
-- Reply-To. NULL when a platform message names nobody, and for proposals made
-- before this column existed. Approved proposals form the owner's history of
-- who he deals with; recording the person, not the platform, means approving
-- one shared document trusts its sharer.
ALTER TABLE triage_proposals ADD COLUMN counterparty TEXT;
