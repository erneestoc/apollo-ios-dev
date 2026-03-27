// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class PetSearchQuery: GraphQLQuery {
  public static let operationName: String = "PetSearch"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query PetSearch($filters: PetSearchFilters = { species: ["Dog", "Cat"] size: SMALL measurements: { height: 10.5, weight: 5.0 } }) { pets(filters: $filters) { __typename id humanName } }"#
    ))

  public var filters: GraphQLNullable<TestSchema.PetSearchFilters>

  public init(filters: GraphQLNullable<TestSchema.PetSearchFilters> = .init(
    TestSchema.PetSearchFilters(
      species: ["Dog", "Cat"],
      size: .init(.SMALL),
      measurements: .init(
        TestSchema.MeasurementsInput(
          height: 10.5,
          weight: 5.0
        )
      )
    )
  )) {
    self.filters = filters
  }

  public var __variables: Variables? { ["filters": filters] }

  public struct Data: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Query }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("pets", [Pet].self, arguments: ["filters": .variable("filters")]),
    ] }

    public var pets: [Pet] { __data["pets"] }

    /// Pet
    public struct Pet: TestSchema.SelectionSet {
      public let __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      public var id: TestSchema.ID { __data["id"] }
      public var humanName: String? { __data["humanName"] }
    }
  }
}
