port module Main exposing (main)

import Browser
import Element exposing (..)
import Element.Background as Background
import Element.Border as Border
import Element.Font as Font
import Element.Input as Input
import Html
import Http
import Json.Encode



-- PORTS


port startWorker : () -> Cmd msg


port subscriptionResultHandler : (String -> msg) -> Sub msg



-- MODEL


type alias Model =
    { subscriptionStatus : SubscriptionStatus
    , reportingBananaFoundStatus : ReportingBananaFoundStatus
    }


type alias Flags =
    { isSubscribed : Bool
    }


type SubscriptionStatus
    = NotSubscribed
    | Subscribing
    | Subscribed
    | SubscriptionFailed
    | NotificationsDenied
    | SubscriptionStatusUnknown String


type ReportingBananaFoundStatus
    = Idle
    | ReportingBananaFound
    | FinishedReportingBananaFound (Result Http.Error ())


init : Flags -> ( Model, Cmd Msg )
init flags =
    ( { subscriptionStatus =
            if flags.isSubscribed then
                Subscribed

            else
                NotSubscribed
      , reportingBananaFoundStatus = Idle
      }
    , Cmd.none
    )



-- UPDATE


type Floor
    = Floor Int


type Msg
    = StartSubscription
    | SubscriptionResultSubscribed
    | SubscriptionResultFailed
    | SubscriptionResultNotificationsDenied
    | SubscriptionResultUnknown String
    | ReportBananaFound Floor -- Send a message to the server which will boradcase it as push messages to everyone
    | ReportBananaFoundResult (Result Http.Error ())


subscriptionResultToMessage : String -> Msg
subscriptionResultToMessage result =
    case result of
        "subscribed" ->
            SubscriptionResultSubscribed

        "failed" ->
            SubscriptionResultFailed

        "notificationsDenied" ->
            SubscriptionResultNotificationsDenied

        other ->
            SubscriptionResultUnknown other


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        StartSubscription ->
            ( { model | subscriptionStatus = Subscribing }, startWorker () )

        SubscriptionResultSubscribed ->
            ( { model | subscriptionStatus = Subscribed }, Cmd.none )

        SubscriptionResultFailed ->
            ( { model | subscriptionStatus = SubscriptionFailed }, Cmd.none )

        SubscriptionResultNotificationsDenied ->
            ( { model | subscriptionStatus = NotificationsDenied }, Cmd.none )

        SubscriptionResultUnknown other ->
            ( { model | subscriptionStatus = SubscriptionStatusUnknown other }, Cmd.none )

        ReportBananaFound (Floor floor) ->
            ( { model | reportingBananaFoundStatus = ReportingBananaFound }
            , Http.post
                { url = "/api/message"

                -- , body = Http.jsonBody (Json.Encode.object [ ( "floor", Json.Encode.int floor ) ])
                , body = Http.stringBody "text/plain" ("Banana found on floor " ++ String.fromInt floor)
                , expect = Http.expectWhatever ReportBananaFoundResult
                }
            )

        ReportBananaFoundResult result ->
            ( { model | reportingBananaFoundStatus = FinishedReportingBananaFound result }, Cmd.none )



-- VIEW


main : Program Flags Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , subscriptions = subscriptions
        , view = view
        }


subscriptions : Model -> Sub Msg
subscriptions _ =
    subscriptionResultHandler subscriptionResultToMessage


makeSubscribeButton : Element Msg
makeSubscribeButton =
    Input.button
        [ Border.rounded 10
        , Border.width 2
        , Border.color (rgb255 255 215 0)
        , paddingXY 24 14
        , centerX
        ]
        { onPress = Just StartSubscription
        , label =
            el
                []
                (text "Subscribe")
        }


subscriptionPanel : Model -> Element Msg
subscriptionPanel model =
    case model.subscriptionStatus of
        NotSubscribed ->
            makeSubscribeButton

        Subscribing ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 255 120)
                , centerX
                ]
                (text "Subscribing...")

        Subscribed ->
            el
                [ Font.size 22
                , Font.color (rgb255 0 255 180)
                , centerX
                ]
                (text "You have subscribed")

        SubscriptionFailed ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text "Subscription failed")

        NotificationsDenied ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text "Notifications permission denied")

        SubscriptionStatusUnknown other ->
            el
                [ Font.size 22
                , Font.color (rgb255 255 100 100)
                , centerX
                ]
                (text ("Subscription status unknown: " ++ other))


view : Model -> Html.Html Msg
view model =
    layout
        [ Background.color (rgb255 35 35 35)
        , Font.color (rgb255 255 255 120)
        ]
    <|
        column
            [ width fill
            , height fill
            , spacing 24
            , centerX
            , centerY
            , padding 24
            ]
            [ el
                [ Font.size 36
                , Font.bold
                , centerX
                ]
                (text "Hello Elm")
            , subscriptionPanel model
            , Input.button
                [ Border.rounded 10
                , Border.width 2
                , Border.color (rgb255 255 215 0)
                , paddingXY 24 14
                , centerX
                ]
                { onPress = Just (ReportBananaFound (Floor 1))
                , label =
                    el
                        []
                        (text "I see bananas in the kitchen!")
                }
            ]
