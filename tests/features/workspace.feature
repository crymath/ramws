Feature: Workspace lifecycle
  Scenario: Editing in RAM syncs back to disk
    Given a temporary project with a tracked file
    And the ramws config is initialized
    When I start the workspace
    And I edit the tracked file inside the workspace
    And I sync changes back to disk
    Then the original project sees the updated contents
